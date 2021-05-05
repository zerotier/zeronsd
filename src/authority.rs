use std::{
    collections::HashMap,
    io::Write,
    net::IpAddr,
    str::FromStr,
    sync::{Arc, Mutex, RwLock},
};

use central::{apis::configuration::Configuration, models::Member};
use cidr_utils::cidr::IpCidr;
use tokio::runtime::Runtime;
use trust_dns_resolver::{
    config::{NameServerConfigGroup, ResolverOpts},
    IntoName,
};
use trust_dns_server::{
    authority::{AuthorityObject, Catalog},
    client::rr::{Name, RData, Record},
    store::forwarder::ForwardAuthority,
    store::{forwarder::ForwardConfig, in_memory::InMemoryAuthority},
};

pub const DOMAIN_NAME: &str = "domain.";

const WHITESPACE_SPLIT: &str = r"\s+";
const COMMENT_MATCH: &str = r"^\s*#";

type Authority = Box<Arc<RwLock<InMemoryAuthority>>>;
type PtrAuthority = Option<Authority>;

pub struct ZTAuthority {
    ptr_authority: PtrAuthority,
    authority: Authority,
    domain_name: Name,
    serial: Arc<Mutex<u32>>,
    ptr_serial: Arc<Mutex<u32>>,
    network: String,
    config: Configuration,
    hosts_file: Option<String>,
}

type HostsFile = HashMap<IpAddr, Vec<String>>;

impl ZTAuthority {
    pub fn new(
        domain_name: Name,
        initial_serial: u32,
        network: String,
        config: Configuration,
        hosts_file: Option<String>,
        listen_ip: String,
    ) -> Result<Arc<Self>, anyhow::Error> {
        let ptr_authority = match IpCidr::from_str(listen_ip)? {
            IpCidr::V4(ip) => {
                let mut s = String::new();

                for octet in ip
                    .get_prefix_as_u8_array()
                    .iter()
                    .rev()
                    .skip((ip.get_bits() / 8) as usize)
                {
                    s += &format!("{}.", octet).to_string();
                }

                Some(Box::new(Arc::new(RwLock::new(InMemoryAuthority::empty(
                    Name::from_str(&s.trim_end_matches("."))?
                        .append_domain(&Name::from_str("in-addr.arpa.")?),
                    trust_dns_server::authority::ZoneType::Primary,
                    false,
                )))))
            }
            IpCidr::V6(_) => None,
        };

        Ok(Arc::new(Self {
            serial: Arc::new(Mutex::new(initial_serial)),
            ptr_serial: Arc::new(Mutex::new(0)),
            domain_name: domain_name.clone(),
            network,
            config,
            hosts_file,
            authority: Box::new(Arc::new(RwLock::new(InMemoryAuthority::empty(
                domain_name.clone(),
                trust_dns_server::authority::ZoneType::Primary,
                false,
            )))),
            ptr_authority,
        }))
    }

    async fn get_members(self: Arc<Self>) -> Result<Vec<Member>, anyhow::Error> {
        let list =
            central::apis::network_member_api::get_network_member_list(&self.config, &self.network)
                .await?;
        Ok(list)
    }

    pub async fn find_members(self: Arc<Self>) {
        loop {
            match self.clone().get_members().await {
                Ok(members) => match self.clone().configure(members) {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("error configuring authority: {}", e)
                    }
                },
                Err(e) => {
                    eprintln!("error syncing members: {}", e)
                }
            }

            tokio::time::sleep(std::time::Duration::new(30, 0)).await;
        }
    }

    pub fn configure(self: Arc<Self>, members: Vec<Member>) -> Result<(), anyhow::Error> {
        self.configure_hosts(self.authority.write().unwrap())?;
        self.configure_members(
            self.authority.write().unwrap(),
            self.ptr_authority.clone(),
            members,
        )
    }

    fn configure_members(
        &self,
        mut authority: std::sync::RwLockWriteGuard<InMemoryAuthority>,
        ptr_authority: PtrAuthority,
        members: Vec<Member>,
    ) -> Result<(), anyhow::Error> {
        for member in members {
            let member_name = format!("zt-{}", member.node_id.unwrap());
            let fqdn = Name::from_str(&member_name)?.append_name(&self.domain_name.clone());
            let mut serial = *(self.serial.lock().unwrap());
            let mut ptr_serial = *(self.ptr_serial.lock().unwrap());

            // this is default the zt-<member id> but can switch to a named name if
            // tweaked in central. see below.
            let mut canonical_name = fqdn.clone();
            let mut member_is_named = false;

            if let Some(name) = member.name.clone() {
                canonical_name = Name::from_str(&name)?.append_name(&self.domain_name.clone());
                member_is_named = true;
            }

            for ip in member.config.unwrap().ip_assignments.unwrap() {
                match IpAddr::from_str(&ip).unwrap() {
                    IpAddr::V4(ip) => {
                        let mut address = Record::with(
                            fqdn.clone(),
                            trust_dns_server::client::rr::RecordType::A,
                            60,
                        );
                        address.set_rdata(RData::A(ip));
                        serial += 1;

                        authority.upsert(address, serial);
                        if member_is_named {
                            let mut address = Record::with(
                                canonical_name.clone(),
                                trust_dns_server::client::rr::RecordType::A,
                                60,
                            );
                            address.set_rdata(RData::A(ip));
                            serial += 1;
                            authority.upsert(address, serial);
                        }

                        if let Some(local_ptr_authority) = ptr_authority.clone() {
                            let mut local_ptr_authority = local_ptr_authority.write().unwrap();
                            let mut ptr = Record::with(
                                ip.into_name()?,
                                trust_dns_server::client::rr::RecordType::PTR,
                                60,
                            );

                            ptr.set_rdata(RData::PTR(canonical_name.clone()));
                            ptr_serial += 1;
                            local_ptr_authority.upsert(ptr, ptr_serial);
                        }
                    }
                    IpAddr::V6(ip) => {
                        let mut address = Record::with(
                            fqdn.clone(),
                            trust_dns_server::client::rr::RecordType::AAAA,
                            60,
                        );
                        address.set_rdata(RData::AAAA(ip));
                        serial += 1;
                        authority.upsert(address, serial);
                    }
                }
            }
        }

        Ok(())
    }

    fn configure_hosts(
        &self,
        mut authority: std::sync::RwLockWriteGuard<InMemoryAuthority>,
    ) -> Result<(), anyhow::Error> {
        for (ip, hostnames) in self.parse_hosts()? {
            for hostname in hostnames {
                let fqdn = Name::from_str(&hostname)?.append_name(&self.domain_name.clone());
                let mut serial = *(self.serial.lock().unwrap());
                match ip {
                    IpAddr::V4(ip) => {
                        let mut address = Record::with(
                            fqdn.clone(),
                            trust_dns_server::client::rr::RecordType::A,
                            60,
                        );
                        address.set_rdata(RData::A(ip));
                        serial += 1;

                        authority.upsert(address, serial);
                    }
                    IpAddr::V6(ip) => {
                        let mut address = Record::with(
                            fqdn.clone(),
                            trust_dns_server::client::rr::RecordType::AAAA,
                            60,
                        );
                        address.set_rdata(RData::AAAA(ip));
                        serial += 1;
                        authority.upsert(address, serial);
                    }
                }
            }
        }
        Ok(())
    }

    fn parse_hosts(&self) -> Result<HostsFile, std::io::Error> {
        let mut input: HostsFile = HashMap::new();

        if let None = self.hosts_file {
            return Ok(input);
        }

        let whitespace = regex::Regex::new(WHITESPACE_SPLIT).unwrap();
        let comment = regex::Regex::new(COMMENT_MATCH).unwrap();
        let content = std::fs::read_to_string(self.hosts_file.clone().unwrap())?;

        for line in content.lines() {
            let mut ary = whitespace.split(line);

            // the first item will be the host
            match ary.next() {
                Some(ip) => {
                    if comment.is_match(ip) {
                        continue;
                    }

                    match IpAddr::from_str(ip) {
                        Ok(parsed_ip) => {
                            let mut v: Vec<String> = Vec::new();

                            // continue to iterate over the addresses
                            for host in ary {
                                v.push(host.into());
                            }

                            input.insert(parsed_ip, v);
                        }
                        Err(e) => {
                            writeln!(std::io::stderr().lock(), "Couldn't parse {}: {}", ip, e)?;
                        }
                    }
                }
                None => {}
            }
        }

        Ok(input)
    }

    pub fn catalog(&self, runtime: &mut Runtime) -> Result<Catalog, std::io::Error> {
        let mut catalog = Catalog::default();
        catalog.upsert(self.domain_name.clone().into(), self.authority.box_clone());
        if self.ptr_authority.is_some() {
            let unwrapped = self.ptr_authority.clone().unwrap();
            catalog.upsert(unwrapped.origin(), unwrapped.box_clone());
        } else {
            println!("PTR records are not supported on IPv6 networks (yet!)");
        }

        let resolvconf = trust_dns_resolver::config::ResolverConfig::default();
        let mut nsconfig = NameServerConfigGroup::new();

        for server in resolvconf.name_servers() {
            nsconfig.push(server.clone());
        }

        let options = Some(ResolverOpts::default());
        let config = &ForwardConfig {
            name_servers: nsconfig.clone(),
            options,
        };

        let forwarder = ForwardAuthority::try_from_config(
            Name::root(),
            trust_dns_server::authority::ZoneType::Primary,
            config,
        );

        let forwarder = runtime.block_on(forwarder).unwrap();

        catalog.upsert(
            Name::root().into(),
            Box::new(Arc::new(RwLock::new(forwarder))),
        );

        Ok(catalog)
    }
}
