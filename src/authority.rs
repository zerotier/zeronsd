use crate::{
    hosts::{parse_hosts, HostsFile},
    utils::parse_member_name,
};

use std::{
    net::IpAddr,
    str::FromStr,
    sync::{Arc, RwLock},
    time::Duration,
};

use cidr_utils::cidr::IpCidr;
use tokio::runtime::Runtime;
use trust_dns_resolver::{
    config::{NameServerConfigGroup, ResolverOpts},
    proto::rr::dnssec::SupportedAlgorithms,
    IntoName,
};
use trust_dns_server::{
    authority::{AuthorityObject, Catalog},
    client::rr::{Name, RData, Record, RecordType, RrKey},
    store::forwarder::ForwardAuthority,
    store::{forwarder::ForwardConfig, in_memory::InMemoryAuthority},
};
use zerotier_central_api::{apis::configuration::Configuration, models::Member};

type Authority = Box<Arc<RwLock<InMemoryAuthority>>>;
type PtrAuthority = Option<Authority>;

fn prune_records(
    authority: &mut std::sync::RwLockWriteGuard<InMemoryAuthority>,
    written: Vec<Name>,
    hosts: Box<HostsFile>,
) -> Result<(), anyhow::Error> {
    let mut rrkey_list = Vec::new();
    let rr = authority.records_mut();

    for (rrkey, _) in rr.clone() {
        let key = &rrkey.name().into_name()?;
        if !written.contains(key)
            && !hosts
                .values()
                .flatten()
                .any(|v| v.to_string().eq(&key.to_string()))
        {
            rrkey_list.push(rrkey);
        }
    }

    for rrkey in rrkey_list {
        eprintln!("Removing expired record {}", rrkey.name());
        rr.remove(&rrkey);
    }

    Ok(())
}

fn upsert_address(
    authority: &mut std::sync::RwLockWriteGuard<InMemoryAuthority>,
    fqdn: Name,
    rt: RecordType,
    rdata: RData,
) {
    let mut address = Record::with(fqdn.clone(), rt, 60);
    address.set_rdata(rdata);
    let serial = authority.serial() + 1;
    authority.upsert(address, serial);
}

fn set_ptr_record(
    authority: &mut std::sync::RwLockWriteGuard<InMemoryAuthority>,
    ip_name: Name,
    canonical_name: Name,
) {
    eprintln!(
        "Replacing PTR record {}: ({})",
        ip_name.clone(),
        canonical_name
    );

    authority.records_mut().remove(&RrKey::new(
        ip_name.clone().into_name().unwrap().into(),
        RecordType::PTR,
    ));

    upsert_address(
        authority,
        ip_name.clone(),
        RecordType::PTR,
        RData::PTR(canonical_name),
    );
}

fn set_ip_record(
    authority: &mut std::sync::RwLockWriteGuard<InMemoryAuthority>,
    name: Name,
    newip: IpAddr,
) {
    eprintln!("Adding new record {}: ({})", name.clone(), &newip);

    match newip {
        IpAddr::V4(newip) => {
            upsert_address(authority, name.clone(), RecordType::A, RData::A(newip));
        }
        IpAddr::V6(newip) => {
            upsert_address(
                authority,
                name.clone(),
                RecordType::AAAA,
                RData::AAAA(newip),
            );
        }
    }
}

fn configure_ptr(
    mut authority: std::sync::RwLockWriteGuard<InMemoryAuthority>,
    ip: IpAddr,
    canonical_name: Name,
) -> Result<(), anyhow::Error> {
    match authority
        .records()
        .get(&RrKey::new(ip.into_name()?.into(), RecordType::PTR))
    {
        Some(records) => {
            if !records
                .records(false, SupportedAlgorithms::all())
                .any(|rec| rec.rdata().eq(&RData::PTR(canonical_name.clone())))
            {
                set_ptr_record(&mut authority, ip.into_name()?, canonical_name.clone());
            }
        }
        None => set_ptr_record(&mut authority, ip.into_name()?, canonical_name.clone()),
    }
    Ok(())
}

pub struct ZTAuthority {
    ptr_authority: PtrAuthority,
    authority: Authority,
    domain_name: Name,
    network: String,
    config: Configuration,
    hosts: Box<HostsFile>,
    update_interval: Duration,
}

impl ZTAuthority {
    pub fn new(
        domain_name: Name,
        network: String,
        config: Configuration,
        hosts_file: Option<String>,
        listen_ip: String,
        update_interval: Duration,
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
            update_interval,
            domain_name: domain_name.clone(),
            network,
            config,
            authority: Box::new(Arc::new(RwLock::new(InMemoryAuthority::empty(
                domain_name.clone(),
                trust_dns_server::authority::ZoneType::Primary,
                false,
            )))),
            ptr_authority,
            hosts: Box::new(parse_hosts(hosts_file.clone(), domain_name.clone())?),
        }))
    }

    async fn get_members(self: Arc<Self>) -> Result<Vec<Member>, anyhow::Error> {
        let list = zerotier_central_api::apis::network_member_api::get_network_member_list(
            &self.config,
            &self.network,
        )
        .await?;
        Ok(list)
    }

    pub async fn find_members(self: Arc<Self>) {
        self.configure_hosts(self.authority.write().unwrap())
            .unwrap();

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

            tokio::time::sleep(self.update_interval).await;
        }
    }

    pub fn configure(self: Arc<Self>, members: Vec<Member>) -> Result<(), anyhow::Error> {
        self.configure_members(
            self.authority.write().unwrap(),
            self.ptr_authority.clone(),
            members,
        )
    }

    fn match_or_insert(
        &self,
        authority: &mut std::sync::RwLockWriteGuard<InMemoryAuthority>,
        name: Name,
        rt: RecordType,
        newip: IpAddr,
    ) {
        match authority
            .records()
            .get(&RrKey::new(name.clone().into(), rt))
        {
            Some(records) => {
                let rdata = match newip {
                    IpAddr::V4(ip) => RData::A(ip),
                    IpAddr::V6(ip) => RData::AAAA(ip),
                };

                if !records
                    .records(false, SupportedAlgorithms::all())
                    .any(|rec| rec.rdata().eq(&rdata))
                {
                    set_ip_record(authority, name, newip);
                }
            }
            None => set_ip_record(authority, name, newip),
        }
    }

    fn configure_members(
        &self,
        mut authority: std::sync::RwLockWriteGuard<InMemoryAuthority>,
        ptr_authority: PtrAuthority,
        members: Vec<Member>,
    ) -> Result<(), anyhow::Error> {
        let mut records = Vec::new();
        let mut ptr_records = Vec::new();

        for member in members {
            let member_name = format!("zt-{}", member.node_id.unwrap());
            let fqdn = Name::from_str(&member_name)?.append_name(&self.domain_name.clone());

            // this is default the zt-<member id> but can switch to a named name if
            // tweaked in central. see below.
            let mut canonical_name = fqdn.clone();
            let mut member_is_named = false;

            if let Some(name) = parse_member_name(member.name.clone()) {
                canonical_name = name.append_name(&self.domain_name.clone());
                member_is_named = true;
            }

            for ip in member.config.unwrap().ip_assignments.unwrap() {
                let ip = IpAddr::from_str(&ip).unwrap();
                records.push(fqdn.clone());

                if member_is_named {
                    records.push(canonical_name.clone());
                }

                match ip {
                    IpAddr::V4(_) => {
                        self.match_or_insert(&mut authority, fqdn.clone(), RecordType::A, ip);
                        if member_is_named {
                            self.match_or_insert(
                                &mut authority,
                                canonical_name.clone(),
                                RecordType::A,
                                ip,
                            );
                        }
                    }
                    IpAddr::V6(_) => {
                        self.match_or_insert(&mut authority, fqdn.clone(), RecordType::AAAA, ip);
                        if member_is_named {
                            self.match_or_insert(
                                &mut authority,
                                canonical_name.clone(),
                                RecordType::AAAA,
                                ip,
                            );
                        }
                    }
                }

                if let Some(local_ptr_authority) = ptr_authority.clone() {
                    ptr_records.push(ip.into_name()?);
                    configure_ptr(
                        local_ptr_authority.write().unwrap(),
                        ip,
                        canonical_name.clone(),
                    )?;
                }
            }
        }

        prune_records(&mut authority, records, self.hosts.clone())?;

        if let Some(ptr_authority) = ptr_authority {
            prune_records(
                &mut ptr_authority.write().unwrap(),
                ptr_records,
                self.hosts.clone(),
            )?;
        }

        Ok(())
    }

    fn configure_hosts(
        &self,
        mut authority: std::sync::RwLockWriteGuard<InMemoryAuthority>,
    ) -> Result<(), anyhow::Error> {
        for (ip, hostnames) in self.hosts.iter() {
            for hostname in hostnames {
                set_ip_record(&mut authority, hostname.clone(), *ip);
            }
        }
        Ok(())
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

#[cfg(test)]
mod tests;
