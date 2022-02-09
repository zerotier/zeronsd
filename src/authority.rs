use std::{
    collections::HashMap,
    net::IpAddr,
    path::PathBuf,
    str::FromStr,
    sync::{Arc, RwLock, RwLockWriteGuard},
    time::Duration,
};

use ipnetwork::IpNetwork;
use log::{error, info, warn};
use tokio::sync::RwLockReadGuard;
use trust_dns_resolver::{
    config::{NameServerConfigGroup, ResolverOpts},
    proto::{
        error::ProtoError,
        rr::{dnssec::SupportedAlgorithms, rdata::SOA, RecordSet},
    },
    IntoName,
};

use trust_dns_server::{
    authority::{AuthorityObject, Catalog},
    client::rr::{LowerName, Name, RData, Record, RecordType, RrKey},
    store::forwarder::ForwardAuthority,
    store::{forwarder::ForwardConfig, in_memory::InMemoryAuthority},
};
use zerotier_central_api::{
    apis::configuration::Configuration,
    models::{Member, Network},
};

use crate::{
    addresses::Calculator,
    hosts::{parse_hosts, HostsFile},
    utils::{parse_member_name, ToHostname},
};

pub(crate) trait ToPointerSOA {
    fn to_ptr_soa_name(self) -> Result<Name, ProtoError>;
}

impl ToPointerSOA for IpNetwork {
    fn to_ptr_soa_name(self) -> Result<Name, ProtoError> {
        // how many bits in each ptr octet
        let octet_factor = match self {
            IpNetwork::V4(_) => 8,
            IpNetwork::V6(_) => 4,
        };

        Ok(self
            .network()
            .into_name()?
            // round off the subnet, account for in-addr.arpa.
            .trim_to((self.prefix() as usize / octet_factor) + 2))
    }
}

pub(crate) trait ToWildcard {
    fn to_wildcard(self, count: u8) -> Name;
}

impl ToWildcard for Name {
    fn to_wildcard(self, count: u8) -> Name {
        let mut name = Self::from_str("*").unwrap();
        for _ in 0..count {
            name = name.append_domain(&Self::from_str("*").unwrap());
        }

        name.append_domain(&self).into_wildcard()
    }
}

pub(crate) type TokioZTAuthority = Arc<tokio::sync::RwLock<ZTAuthority>>;
// Authority is lock managed, and kept on the heap. Be mindful when modifying through the Arc.
pub(crate) type Authority = Box<Arc<RwLock<InMemoryAuthority>>>;
pub(crate) type PtrAuthorityMap = HashMap<IpNetwork, Authority>;

pub(crate) fn new_ptr_authority(ip: IpNetwork) -> Result<Authority, anyhow::Error> {
    let domain_name = ip.to_ptr_soa_name()?;

    let mut authority = InMemoryAuthority::empty(
        domain_name.clone(),
        trust_dns_server::authority::ZoneType::Primary,
        false,
    );

    set_soa(&mut authority, domain_name);

    Ok(Box::new(Arc::new(RwLock::new(authority))))
}

// find_members waits for the update_interval time, then populates the authority based on the
// members list. This call is fairly high level; most of the other calls are called by it
// indirectly.
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
            Ok((network, members)) => match zt.write().await.configure_members(network, members) {
                Ok(_) => {}
                Err(e) => {
                    error!("error configuring authority: {}", e)
                }
            },
            Err(e) => {
                error!("error syncing members: {}", e)
            }
        }
    }
}

// get_members is a convenience method for the openapi calls required to get member and network
// information. It could be named better.
async fn get_members(
    zt: RwLockReadGuard<'_, ZTAuthority>,
) -> Result<(Network, Vec<Member>), anyhow::Error> {
    let config = zt.config.clone();
    let network = zt.network.clone();

    let members =
        zerotier_central_api::apis::network_member_api::get_network_member_list(&config, &network)
            .await?;

    let network =
        zerotier_central_api::apis::network_api::get_network_by_id(&config, &network).await?;

    Ok((network, members))
}

// prune_records walks the authority and a tracked list of the most recent writes. Then, it
// subtracts the items in the authority that are not in the tracked list.
fn prune_records(authority: Authority, written: Vec<Name>) -> Result<(), anyhow::Error> {
    let mut rrkey_list = Vec::new();
    let mut lock = authority
        .write()
        .expect("Could not acquire write lock on authority");

    let rr = lock.records_mut();

    for (rrkey, rs) in rr.clone() {
        let key = &rrkey.name().into_name()?;
        if !written.contains(key) && rs.record_type() != RecordType::SOA {
            rrkey_list.push(rrkey);
        }
    }

    for rrkey in rrkey_list {
        warn!("Removing expired record {}", rrkey.name());
        rr.remove(&rrkey);
    }

    Ok(())
}

// upsert_address is a convenience function to transform and insert any record, v4, v6, and ptr
// variants.
fn upsert_address(
    authority: &mut RwLockWriteGuard<InMemoryAuthority>,
    fqdn: Name,
    rt: RecordType,
    rdatas: Vec<RData>,
) {
    let serial = authority.serial() + 1;
    let records = authority.records_mut();
    let key = RrKey::new(LowerName::from(fqdn.clone()), rt);

    records.remove(&key);

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
            info!("Adding new record {}: ({})", fqdn.clone(), rdata.clone());
            authority.upsert(address, serial);
        }
    }
}

// set_ptr_record transforms member information into a ptr record.
fn set_ptr_record(
    authority: &mut RwLockWriteGuard<InMemoryAuthority>,
    ip_name: Name,
    canonical_name: Name,
) {
    warn!(
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

// replace_ip_record replaces an IP record of either v4 or v6 type.
fn replace_ip_record(
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

// configure_ptr is a frontend for set_ptr_record that acquires locks and other stuff.
fn configure_ptr(
    authority: Authority,
    ip: Name,
    canonical_name: Name,
) -> Result<(), anyhow::Error> {
    let lock = &mut authority
        .write()
        .expect("Could not acquire authority write lock");

    match lock
        .records()
        .get(&RrKey::new(ip.clone().into(), RecordType::PTR))
    {
        Some(records) => {
            if !records
                .records(false, SupportedAlgorithms::all())
                .any(|rec| rec.rdata().eq(&RData::PTR(canonical_name.clone())))
            {
                set_ptr_record(lock, ip.clone(), canonical_name.clone());
            }
        }
        None => set_ptr_record(lock, ip.clone(), canonical_name.clone()),
    }
    Ok(())
}

// set_soa should only be called once per authority; it configures the SOA record for the zone.
pub(crate) fn set_soa(authority: &mut InMemoryAuthority, domain_name: Name) {
    let mut soa = Record::new();
    soa.set_name(domain_name.clone());
    soa.set_rr_type(RecordType::SOA);
    soa.set_rdata(RData::SOA(SOA::new(
        domain_name.clone(),
        Name::from_str("administrator")
            .unwrap()
            .append_domain(&domain_name.clone()),
        authority.serial() + 1,
        30,
        0,
        -1,
        0,
    )));
    authority.upsert(soa, authority.serial() + 1);
}

// init_trust_dns_authority is a really ugly constructor.
pub(crate) fn init_trust_dns_authority(domain_name: Name) -> Authority {
    let mut authority = InMemoryAuthority::empty(
        domain_name.clone(),
        trust_dns_server::authority::ZoneType::Primary,
        false,
    );

    set_soa(&mut authority, domain_name.clone());
    Box::new(Arc::new(RwLock::new(authority)))
}

// init_catalog: also a really ugly constructor, but in this case initializes the whole trust-dns
// subsystem.
pub(crate) async fn init_catalog(zt: TokioZTAuthority) -> Result<Catalog, anyhow::Error> {
    let read = zt.read().await;

    let mut catalog = Catalog::default();
    catalog.upsert(read.domain_name.clone().into(), read.authority.box_clone());
    for (network, authority) in read.ptr_authority_map.clone() {
        catalog.upsert(network.to_ptr_soa_name()?.into(), authority.box_clone());
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

// ZTRecord is the encapsulation of a single record; Members are usually transformed into this
// struct.
pub(crate) struct ZTRecord {
    fqdn: Name,
    canonical_name: Option<Name>,
    ptr_name: Name,
    ips: Vec<IpAddr>,
    wildcard_everything: bool,
    member: Member,
}

impl ZTRecord {
    pub(crate) fn new(
        member: &Member,
        sixplane: Option<IpNetwork>,
        rfc4193: Option<IpNetwork>,
        domain_name: Name,
        wildcard_everything: bool,
    ) -> Result<Self, anyhow::Error> {
        let member_name = format!(
            "zt-{}",
            member
                .clone()
                .node_id
                .expect("Node ID for member does not exist")
        );

        let fqdn = member_name.clone().to_fqdn(domain_name.clone())?;

        // this is default the zt-<member id> but can switch to a named name if
        // tweaked in central. see below.
        let mut canonical_name = None;
        let mut ptr_name = fqdn.clone();

        if let Some(name) = parse_member_name(member.name.clone(), domain_name.clone()) {
            canonical_name = Some(name.clone());
            ptr_name = name;
        }

        let mut ips: Vec<IpAddr> = member
            .clone()
            .config
            .expect("Member config does not exist")
            .ip_assignments
            .expect("IP assignments for member do not exist")
            .into_iter()
            .map(|s| IpAddr::from_str(&s).expect("Could not parse IP address"))
            .collect();

        if sixplane.is_some() {
            ips.push(member.clone().sixplane()?.ip());
        }

        if rfc4193.is_some() {
            ips.push(member.clone().rfc4193()?.ip());
        }

        Ok(Self {
            member: member.clone(),
            wildcard_everything,
            fqdn,
            canonical_name,
            ptr_name,
            ips,
        })
    }

    // insert_records is hopefully well-named.
    pub(crate) fn insert_records(&self, records: &mut Vec<Name>) {
        records.push(self.fqdn.clone());

        for ip in self.ips.clone() {
            records.push(ip.into_name().expect("Could not coerce IP into name"))
        }

        if self.canonical_name.is_some() {
            records.push(self.canonical_name.clone().unwrap());
            if self.wildcard_everything {
                records.push(self.get_canonical_wildcard().unwrap())
            }
        }

        if self.wildcard_everything {
            records.push(self.fqdn.clone().to_wildcard(0))
        }
    }

    // get_canonical_wildcard is a function to combine canonical_name (named members) and wildcard functionality.
    pub(crate) fn get_canonical_wildcard(&self) -> Option<Name> {
        self.canonical_name.clone().map(|name| name.to_wildcard(0))
    }

    // insert_authority is not very well named, but performs the function of inserting a ZTRecord
    // into a ZTAuthority.
    pub(crate) fn insert_authority(&self, authority: &ZTAuthority) -> Result<(), anyhow::Error> {
        authority.match_or_insert(self.fqdn.clone(), self.ips.clone());

        if self.wildcard_everything {
            authority.match_or_insert(self.fqdn.clone().to_wildcard(0), self.ips.clone());
        }

        if self.canonical_name.is_some() {
            authority.match_or_insert(self.canonical_name.clone().unwrap(), self.ips.clone());
            if self.wildcard_everything {
                authority.match_or_insert(self.get_canonical_wildcard().unwrap(), self.ips.clone())
            }
        }

        Ok(())
    }

    // insert_member_ptr is a lot like insert_authority, but for PTRs.
    pub(crate) fn insert_member_ptr(
        &self,
        authority_map: PtrAuthorityMap,
        _sixplane: Option<IpNetwork>,
        rfc4193: Option<IpNetwork>,
        records: &mut Vec<Name>,
    ) -> Result<(), anyhow::Error> {
        for ip in self.ips.clone() {
            for (network, authority) in authority_map.clone() {
                if network.contains(ip) {
                    let ip = ip.into_name().expect("Could not coerce IP into name");
                    records.push(ip.clone());
                    configure_ptr(authority.clone(), ip, self.ptr_name.clone())?;
                }
            }
        }

        if let Some(rfc4193) = rfc4193 {
            let ptr = self.member.clone().rfc4193()?.network().into_name()?;

            records.push(ptr.clone());

            if let Some(authority) = authority_map.get(&rfc4193) {
                configure_ptr(authority.clone(), ptr, self.ptr_name.clone())?;
            }
        }

        Ok(())
    }
}

// ZTAuthority is the customized trust-dns authority.
#[derive(Clone)]
pub(crate) struct ZTAuthority {
    ptr_authority_map: PtrAuthorityMap,
    authority: Authority,
    domain_name: Name,
    network: String,
    config: Configuration,
    hosts_file: Option<PathBuf>,
    hosts: Option<Box<HostsFile>>,
    update_interval: Duration,
    wildcard_everything: bool,
}

impl ZTAuthority {
    pub(crate) fn new(
        domain_name: Name,
        network: String,
        config: Configuration,
        hosts_file: Option<PathBuf>,
        ptr_authority_map: PtrAuthorityMap,
        update_interval: Duration,
        authority: Authority,
    ) -> Self {
        Self {
            update_interval,
            domain_name: domain_name.clone(),
            network,
            config,
            authority,
            ptr_authority_map,
            hosts_file,
            hosts: None,
            wildcard_everything: false,
        }
    }

    pub(crate) fn wildcard_everything(&mut self) {
        self.wildcard_everything = true;
    }

    // match_or_insert avoids duplicate names by finding them first and removing them. Contrast
    // it makes heavy use of replace_ip_record to perform this function.
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

            // for the record type, fetch the named record.
            let rs = lock
                .records()
                .get(&RrKey::new(name.clone().into(), rt))
                .clone();

            // gather all the ips (v6 too) for the record.
            let ips: Vec<IpAddr> = newips
                .clone()
                .into_iter()
                .filter(|i| match i {
                    IpAddr::V4(_) => rt == RecordType::A,
                    IpAddr::V6(_) => rt == RecordType::AAAA,
                })
                .collect();

            match rs {
                Some(rs) => {
                    let records = rs.records(false, SupportedAlgorithms::all());
                    if records.is_empty()
                        || !records.into_iter().all(|r| rdatas.contains(r.rdata()))
                    {
                        drop(lock);
                        if !ips.is_empty() {
                            replace_ip_record(
                                &mut self.authority.write().expect("write lock"),
                                name.clone(),
                                rt,
                                ips,
                            );
                        }
                    }
                }
                None => {
                    drop(lock);
                    if !ips.is_empty() {
                        replace_ip_record(
                            &mut self.authority.write().expect("write lock"),
                            name.clone(),
                            rt,
                            ips,
                        );
                    }
                }
            }
        }
    }

    // configure_members merges the hosts lists and members list with allll the network options to
    // basically mutate the authority into shape.
    fn configure_members(
        &mut self,
        network: Network,
        members: Vec<Member>,
    ) -> Result<(), anyhow::Error> {
        let mut records = vec![self.domain_name.clone()];
        if let Some(hosts) = self.hosts.to_owned() {
            self.prune_hosts();
            records.append(&mut hosts.values().flatten().map(|v| v.clone()).collect());
        }

        let (mut sixplane, mut rfc4193) = (None, None);

        let v6assign = network.config.clone().unwrap().v6_assign_mode;
        if v6assign.is_some() {
            let v6assign = v6assign.unwrap().clone();
            if v6assign.var_6plane.unwrap_or(false) {
                let s = network.clone().sixplane()?;
                sixplane = Some(s);
                records.push(s.to_ptr_soa_name()?);
            }

            if v6assign.rfc4193.unwrap_or(false) {
                let s = network.clone().rfc4193()?;
                rfc4193 = Some(s);
                records.push(s.to_ptr_soa_name()?);
            }
        }

        for member in members {
            let record = ZTRecord::new(
                &member,
                sixplane,
                rfc4193,
                self.domain_name.clone(),
                self.wildcard_everything,
            )?;
            record.insert_authority(self)?;
            record.insert_member_ptr(
                self.ptr_authority_map.to_owned(),
                sixplane,
                rfc4193,
                &mut records,
            )?;
            record.insert_records(&mut records);
        }

        prune_records(self.authority.to_owned(), records.clone())?;
        for authority in self.ptr_authority_map.values() {
            prune_records(authority.to_owned(), records.clone())?;
        }

        Ok(())
    }

    // configure_hosts is for /etc/hosts format management
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

    // prune_hosts removes the hosts after a /etc/hosts update is detected
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

                    warn!("Replacing host record for {} with {:?}", key, ips);
                    rr.remove(&rrkey);
                    rr.insert(rrkey, Arc::new(new_rset));
                }
            }
        }
    }
}

#[cfg(all(feature = "integration-tests", test))]
mod service;
#[cfg(all(feature = "integration-tests", test))]
mod tests;
