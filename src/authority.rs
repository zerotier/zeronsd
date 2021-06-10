use std::{
    collections::HashMap,
    net::IpAddr,
    str::FromStr,
    sync::{Arc, RwLock, RwLockWriteGuard},
    time::Duration,
};

use ipnetwork::IpNetwork;
use tokio::sync::RwLockReadGuard;
use trust_dns_resolver::{
    config::{NameServerConfigGroup, ResolverOpts},
    proto::rr::{dnssec::SupportedAlgorithms, RecordSet},
    IntoName,
};

use trust_dns_server::{
    authority::{AuthorityObject, Catalog},
    client::rr::{Name, RData, Record, RecordType, RrKey},
    store::forwarder::ForwardAuthority,
    store::{forwarder::ForwardConfig, in_memory::InMemoryAuthority},
};
use zerotier_central_api::{apis::configuration::Configuration, models::Member};

use crate::{
    hosts::{parse_hosts, HostsFile},
    utils::{parse_member_name, ToHostname},
};

pub(crate) type TokioZTAuthority = Arc<tokio::sync::RwLock<ZTAuthority>>;
pub(crate) type Authority = Box<Arc<RwLock<InMemoryAuthority>>>;
pub(crate) type PtrAuthority = Option<Authority>;

pub(crate) fn new_ptr_authority(ip: IpNetwork) -> Result<PtrAuthority, anyhow::Error> {
    Ok(match ip {
        IpNetwork::V4(ip) => Some(Box::new(Arc::new(RwLock::new(InMemoryAuthority::empty(
            ip.network()
                .into_name()?
                .trim_to((ip.prefix() as usize / 8) + 2),
            trust_dns_server::authority::ZoneType::Primary,
            false,
        ))))),
        IpNetwork::V6(_) => None,
    })
}

pub(crate) async fn find_members(zt: TokioZTAuthority) {
    let read = zt.read().await;
    let mut interval = tokio::time::interval(read.update_interval.clone());
    drop(read);

    loop {
        interval.tick().await;

        zt.write()
            .await
            .configure_hosts()
            .expect("Could not configure authority from hosts file");

        match get_members(zt.read().await).await {
            Ok(members) => match zt.write().await.configure_members(members) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("error configuring authority: {}", e)
                }
            },
            Err(e) => {
                eprintln!("error syncing members: {}", e)
            }
        }
    }
}

async fn get_members(zt: RwLockReadGuard<'_, ZTAuthority>) -> Result<Vec<Member>, anyhow::Error> {
    let config = zt.config.clone();
    let network = zt.network.clone();

    Ok(
        zerotier_central_api::apis::network_member_api::get_network_member_list(&config, &network)
            .await?,
    )
}

fn prune_records(authority: Authority, written: Vec<Name>) -> Result<(), anyhow::Error> {
    let mut rrkey_list = Vec::new();
    let mut lock = authority
        .write()
        .expect("Could not acquire write lock on authority");

    let rr = lock.records_mut();

    for (rrkey, _) in rr.clone() {
        let key = &rrkey.name().into_name()?;
        if !written.contains(key) {
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
    authority: &mut RwLockWriteGuard<InMemoryAuthority>,
    fqdn: Name,
    rt: RecordType,
    rdatas: Vec<RData>,
) {
    let serial = authority.serial() + 1;
    let records = authority.records_mut();
    let key = records
        .into_iter()
        .map(|(key, _)| key)
        .find(|key| key.name().into_name().unwrap().eq(&fqdn));

    if let Some(key) = key {
        let key = key.clone();
        records
            .get_mut(&key)
            .replace(&mut Arc::new(RecordSet::new(&fqdn.clone(), rt, serial)));
    }

    for rdata in rdatas {
        if match rt {
            RecordType::A => match rdata {
                RData::A(_) => Some(()),
                _ => None,
            },
            RecordType::AAAA => match rdata {
                RData::AAAA(_) => Some(()),
                _ => None,
            },
            RecordType::PTR => match rdata {
                RData::PTR(_) => Some(()),
                _ => None,
            },
            _ => None,
        }
        .is_some()
        {
            let mut address = Record::with(fqdn.clone(), rt, 60);
            address.set_rdata(rdata.clone());
            eprintln!("Adding new record {}: ({})", fqdn.clone(), rdata.clone());
            authority.upsert(address, serial);
        }
    }
}

fn set_ptr_record(
    authority: &mut RwLockWriteGuard<InMemoryAuthority>,
    ip_name: Name,
    canonical_name: Name,
) {
    eprintln!(
        "Replacing PTR record {}: ({})",
        ip_name.clone(),
        canonical_name
    );

    authority.records_mut().remove(&RrKey::new(
        ip_name
            .clone()
            .into_name()
            .expect("Could not coerce IP address into DNS name")
            .into(),
        RecordType::PTR,
    ));

    upsert_address(
        authority,
        ip_name.clone(),
        RecordType::PTR,
        vec![RData::PTR(canonical_name)],
    );
}

fn set_ip_record(
    authority: &mut RwLockWriteGuard<InMemoryAuthority>,
    name: Name,
    rt: RecordType,
    newips: Vec<IpAddr>,
) {
    upsert_address(
        authority,
        name.clone(),
        rt,
        newips
            .into_iter()
            .map(|i| match i {
                IpAddr::V4(i) => RData::A(i),
                IpAddr::V6(i) => RData::AAAA(i),
            })
            .collect(),
    );
}

fn configure_ptr(
    authority: Authority,
    ip: IpAddr,
    canonical_name: Name,
) -> Result<(), anyhow::Error> {
    let lock = &mut authority
        .write()
        .expect("Could not acquire authority write lock");

    match lock
        .records()
        .get(&RrKey::new(ip.into_name()?.into(), RecordType::PTR))
    {
        Some(records) => {
            if !records
                .records(false, SupportedAlgorithms::all())
                .any(|rec| rec.rdata().eq(&RData::PTR(canonical_name.clone())))
            {
                set_ptr_record(lock, ip.into_name()?, canonical_name.clone());
            }
        }
        None => set_ptr_record(lock, ip.into_name()?, canonical_name.clone()),
    }
    Ok(())
}

pub(crate) fn init_trust_dns_authority(domain_name: Name) -> Authority {
    Box::new(Arc::new(RwLock::new(InMemoryAuthority::empty(
        domain_name.clone(),
        trust_dns_server::authority::ZoneType::Primary,
        false,
    ))))
}

pub(crate) async fn init_catalog(zt: TokioZTAuthority) -> Result<Catalog, std::io::Error> {
    let read = zt.read().await;

    let mut catalog = Catalog::default();
    catalog.upsert(read.domain_name.clone().into(), read.authority.box_clone());

    if read.ptr_authority.is_some() {
        let unwrapped = read.ptr_authority.clone().unwrap();
        catalog.upsert(unwrapped.origin(), unwrapped.box_clone());
    } else {
        println!("PTR records are not supported on IPv6 networks (yet!)");
    }

    drop(read);

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
    )
    .await
    .expect("Could not initialize forwarder");

    catalog.upsert(
        Name::root().into(),
        Box::new(Arc::new(RwLock::new(forwarder))),
    );

    Ok(catalog)
}

#[derive(Clone)]
pub(crate) struct ZTAuthority {
    ptr_authority: PtrAuthority,
    authority: Authority,
    domain_name: Name,
    network: String,
    config: Configuration,
    hosts_file: Option<String>,
    hosts: Option<Box<HostsFile>>,
    update_interval: Duration,
}

impl ZTAuthority {
    pub(crate) fn new(
        domain_name: Name,
        network: String,
        config: Configuration,
        hosts_file: Option<String>,
        ptr_authority: PtrAuthority,
        update_interval: Duration,
        authority: Authority,
    ) -> Self {
        Self {
            update_interval,
            domain_name: domain_name.clone(),
            network,
            config,
            authority,
            ptr_authority,
            hosts_file,
            hosts: None,
        }
    }

    fn match_or_insert(&self, name: Name, newips: Vec<IpAddr>) {
        let rdatas: Vec<RData> = newips
            .clone()
            .into_iter()
            .map(|ip| match ip {
                IpAddr::V4(ip) => RData::A(ip),
                IpAddr::V6(ip) => RData::AAAA(ip),
            })
            .collect();

        for rt in vec![RecordType::A, RecordType::AAAA] {
            let lock = self
                .authority
                .read()
                .expect("Could not get authority read lock");
            let records = lock
                .records()
                .get(&RrKey::new(name.clone().into(), rt))
                .clone();

            match records {
                Some(records) => {
                    let records = records.records(false, SupportedAlgorithms::all());
                    if records.is_empty()
                        || !records.into_iter().all(|r| rdatas.contains(r.rdata()))
                    {
                        drop(lock);
                        set_ip_record(
                            &mut self.authority.write().expect("write lock"),
                            name.clone(),
                            rt,
                            newips.clone(),
                        );
                    }
                }
                None => {
                    drop(lock);
                    set_ip_record(
                        &mut self.authority.write().expect("write lock"),
                        name.clone(),
                        rt,
                        newips.clone(),
                    );
                }
            }
        }
    }

    fn configure_members(&mut self, members: Vec<Member>) -> Result<(), anyhow::Error> {
        let mut records = Vec::new();
        if let Some(hosts) = self.hosts.to_owned() {
            self.prune_hosts();
            records.append(&mut hosts.values().flatten().map(|v| v.clone()).collect());
        }

        for member in members {
            let member_name = format!(
                "zt-{}",
                member.node_id.expect("Node ID for member does not exist")
            );
            let fqdn = member_name.to_fqdn(self.domain_name.clone())?;

            // this is default the zt-<member id> but can switch to a named name if
            // tweaked in central. see below.
            let mut canonical_name = fqdn.clone();
            let mut member_is_named = false;

            if let Some(name) = parse_member_name(member.name.clone(), self.domain_name.clone()) {
                canonical_name = name;
                member_is_named = true;
            }

            let ips: Vec<IpAddr> = member
                .config
                .expect("Member config does not exist")
                .ip_assignments
                .expect("IP assignments for member do not exist")
                .into_iter()
                .map(|s| IpAddr::from_str(&s).expect("Could not parse IP address"))
                .collect();

            self.match_or_insert(fqdn.clone(), ips.clone());

            if member_is_named {
                self.match_or_insert(canonical_name.clone(), ips.clone());
            }

            if let Some(local_ptr_authority) = self.ptr_authority.to_owned() {
                for ip in ips.clone() {
                    records.push(ip.into_name().expect("Could not coerce IP into name"));
                    configure_ptr(local_ptr_authority.clone(), ip, canonical_name.clone())?;
                }
            }

            records.push(fqdn.clone());
            records.push(canonical_name.clone());
        }

        prune_records(self.authority.to_owned(), records.clone())?;

        if let Some(ptr_authority) = self.ptr_authority.to_owned() {
            prune_records(ptr_authority, records.clone())?;
        }

        Ok(())
    }

    fn configure_hosts(&mut self) -> Result<(), anyhow::Error> {
        self.hosts = Some(Box::new(parse_hosts(
            self.hosts_file.clone(),
            self.domain_name.clone(),
        )?));

        for (ip, hostnames) in self.hosts.clone().unwrap().iter() {
            for hostname in hostnames {
                self.match_or_insert(hostname.clone(), vec![ip.clone()]);
            }
        }
        Ok(())
    }

    fn prune_hosts(&mut self) {
        if self.hosts.is_none() {
            return;
        }

        let mut lock = self.authority.write().expect("Pruning hosts write lock");

        let serial = lock.serial();
        let rr = lock.records_mut();

        let mut hosts_map = HashMap::new();

        for (ip, hosts) in self.hosts.to_owned().unwrap().into_iter() {
            for host in hosts {
                if !hosts_map.contains_key(&host) {
                    hosts_map.insert(host.clone(), vec![]);
                }

                hosts_map.get_mut(&host).unwrap().push(ip);
            }
        }

        for (host, ips) in hosts_map.into_iter() {
            for (rrkey, rset) in rr.clone() {
                let key = &rrkey.name().into_name().expect("could not parse name");
                let records = rset.records(false, SupportedAlgorithms::all());

                let rt = rset.record_type();
                let rdatas: Vec<RData> = ips
                    .clone()
                    .into_iter()
                    .filter_map(|i| match i {
                        IpAddr::V4(ip) => {
                            if rt == RecordType::A {
                                Some(RData::A(ip))
                            } else {
                                None
                            }
                        }
                        IpAddr::V6(ip) => {
                            if rt == RecordType::AAAA {
                                Some(RData::AAAA(ip))
                            } else {
                                None
                            }
                        }
                    })
                    .collect();

                if key.eq(&host)
                    && (records.is_empty()
                        || !records.map(|r| r.rdata()).all(|rd| rdatas.contains(rd)))
                {
                    let mut new_rset = RecordSet::new(key, rt, serial + 1);
                    for rdata in rdatas.clone() {
                        new_rset.add_rdata(rdata);
                    }

                    eprintln!("Replacing host record for {} with {:?}", key, ips);
                    rr.remove(&rrkey);
                    rr.insert(rrkey, Arc::new(new_rset));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
