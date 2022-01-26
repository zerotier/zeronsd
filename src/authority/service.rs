/*
 * Service abstraction provides a way to automatically generate services that are attached to
 * TestNetworks for testing against. Each Service is composed of a DNS service attached to a
 * TestNetwork. The service can then be resolved against, for example. Several parameters for
 * managing the underlying TestNetwork, and the Service are available via the ServiceConfig struct.
 */

use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    str::FromStr,
    sync::{Arc, Mutex},
    thread::sleep,
    time::Duration,
};

use ipnetwork::IpNetwork;
use log::info;
use rand::prelude::{IteratorRandom, SliceRandom};
use tokio::{runtime::Runtime, sync::RwLock};
use trust_dns_resolver::{
    config::{NameServerConfig, ResolverConfig, ResolverOpts},
    proto::rr::RecordType,
    Resolver,
};

use crate::{
    addresses::Calculator,
    authority::{find_members, init_trust_dns_authority, new_ptr_authority, ZTAuthority},
    integration_tests::{init_test_runtime, TestContext, TestNetwork},
    tests::HOSTS_DIR,
    utils::{authtoken_path, domain_or_default, get_listen_ips, parse_ip_from_cidr},
};

#[derive(Clone)]
pub(crate) struct Service {
    runtime: Arc<Mutex<Runtime>>,
    tn: Arc<TestNetwork>,
    resolvers: Arc<Vec<Arc<Resolver>>>,
    update_interval: Option<Duration>,
    pub listen_ips: Vec<SocketAddr>,
}

pub(crate) trait Lookup {
    fn lookup_a(&self, record: String) -> Vec<Ipv4Addr>;
    fn lookup_aaaa(&self, record: String) -> Vec<Ipv6Addr>;
    fn lookup_ptr(&self, record: String) -> Vec<String>;
}

impl Lookup for Resolver {
    fn lookup_a(&self, record: String) -> Vec<Ipv4Addr> {
        self.lookup(record, RecordType::A)
            .unwrap()
            .record_iter()
            .map(|r| r.rdata().clone().into_a().unwrap())
            .collect()
    }

    fn lookup_aaaa(&self, record: String) -> Vec<Ipv6Addr> {
        self.ipv6_lookup(record)
            .unwrap()
            .as_lookup()
            .record_iter()
            .map(|r| r.rdata().clone().into_aaaa().unwrap())
            .collect()
    }

    fn lookup_ptr(&self, record: String) -> Vec<String> {
        self.lookup(record, RecordType::PTR)
            .unwrap()
            .record_iter()
            .map(|r| r.rdata().clone().into_ptr().unwrap().to_string())
            .collect()
    }
}

pub(crate) enum HostsType {
    Path(&'static str),
    Fixture(&'static str),
    None,
}

fn create_listeners(
    runtime: Arc<Mutex<Runtime>>,
    tn: &TestNetwork,
    hosts: HostsType,
    update_interval: Option<Duration>,
    wildcard_everything: bool,
) -> Vec<SocketAddr> {
    let listen_cidrs = runtime
        .lock()
        .unwrap()
        .block_on(get_listen_ips(
            &authtoken_path(None),
            &tn.network.clone().id.unwrap(),
        ))
        .unwrap();

    let mut listen_ips = Vec::new();

    let mut ipmap = HashMap::new();
    let mut authority_map = HashMap::new();
    let authority = init_trust_dns_authority(domain_or_default(None).unwrap());

    for cidr in listen_cidrs.clone() {
        let listen_ip = parse_ip_from_cidr(cidr.clone());
        let socket_addr = SocketAddr::new(listen_ip.clone(), 53);
        listen_ips.push(socket_addr);
        let cidr = IpNetwork::from_str(&cidr.clone()).unwrap();
        if !ipmap.contains_key(&listen_ip) {
            ipmap.insert(listen_ip, cidr.network());
        }

        if !authority_map.contains_key(&cidr) {
            let ptr_authority = new_ptr_authority(cidr).unwrap();
            authority_map.insert(cidr, ptr_authority.clone());
        }
    }

    if let Some(v6assign) = tn.network.config.clone().unwrap().v6_assign_mode {
        if v6assign.rfc4193.unwrap_or(false) {
            let cidr = tn.network.clone().rfc4193().unwrap();
            if !authority_map.contains_key(&cidr) {
                let ptr_authority = new_ptr_authority(cidr).unwrap();
                authority_map.insert(cidr, ptr_authority);
            }
        }
    }

    let update_interval = update_interval.unwrap_or(Duration::new(2, 0));

    let mut ztauthority = ZTAuthority::new(
        domain_or_default(None).unwrap(),
        tn.network.clone().id.unwrap(),
        tn.central(),
        match hosts {
            HostsType::Fixture(hosts) => Some(format!("{}/{}", HOSTS_DIR, hosts)),
            HostsType::Path(hosts) => Some(hosts.to_string()),
            HostsType::None => None,
        },
        authority_map,
        update_interval,
        authority.clone(),
    );

    if wildcard_everything {
        ztauthority.wildcard_everything();
    }

    let arc_authority = Arc::new(RwLock::new(ztauthority));
    let lock = runtime.lock().unwrap();
    lock.spawn(find_members(arc_authority.clone()));

    lock.block_on(async { tokio::time::sleep(Duration::new(5, 0)).await });

    for ip in listen_ips.clone() {
        let server = crate::server::Server::new(arc_authority.to_owned());
        info!("Serving {}", ip.clone());
        lock.spawn(server.listen(ip, Duration::new(0, 1000)));
    }

    listen_ips
}

fn create_resolvers(sockets: Vec<SocketAddr>) -> Vec<Arc<Resolver>> {
    let mut resolvers = Vec::new();

    for socket in sockets {
        let mut resolver_config = ResolverConfig::new();
        resolver_config.add_search(domain_or_default(None).unwrap());
        resolver_config.add_name_server(NameServerConfig {
            socket_addr: socket,
            protocol: trust_dns_resolver::config::Protocol::Udp,
            tls_dns_name: None,
            trust_nx_responses: true,
        });

        let mut opts = ResolverOpts::default();
        opts.cache_size = 0;
        opts.rotate = true;
        opts.use_hosts_file = false;
        opts.positive_min_ttl = Some(Duration::new(0, 0));
        opts.positive_max_ttl = Some(Duration::new(0, 0));
        opts.negative_min_ttl = Some(Duration::new(0, 0));
        opts.negative_max_ttl = Some(Duration::new(0, 0));

        resolvers.push(Arc::new(
            trust_dns_resolver::Resolver::new(resolver_config, opts).unwrap(),
        ));
    }

    resolvers
}

pub(crate) struct ServiceConfig {
    hosts: HostsType,
    update_interval: Option<Duration>,
    ips: Option<Vec<&'static str>>,
    wildcard_everything: bool,
    network_filename: Option<&'static str>,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            network_filename: None,
            hosts: HostsType::None,
            update_interval: None,
            ips: None,
            wildcard_everything: false,
        }
    }
}

impl ServiceConfig {
    pub(crate) fn network_filename(mut self, n: &'static str) -> Self {
        self.network_filename = Some(n);
        self
    }

    pub(crate) fn hosts(mut self, h: HostsType) -> Self {
        self.hosts = h;
        self
    }

    pub(crate) fn update_interval(mut self, u: Option<Duration>) -> Self {
        self.update_interval = u;
        self
    }

    pub(crate) fn ips(mut self, ips: Option<Vec<&'static str>>) -> Self {
        self.ips = ips;
        self
    }

    pub(crate) fn wildcard_everything(mut self, w: bool) -> Self {
        self.wildcard_everything = w;
        self
    }
}

impl Service {
    pub(crate) fn new(sc: ServiceConfig) -> Self {
        let runtime = init_test_runtime();

        let network_filename = sc.network_filename.unwrap_or("basic-ipv4");
        let tn = if let Some(ips) = sc.ips {
            TestNetwork::new_multi_ip(
                runtime.clone(),
                network_filename,
                &mut TestContext::default(),
                ips,
            )
            .unwrap()
        } else {
            TestNetwork::new(
                runtime.clone(),
                network_filename,
                &mut TestContext::default(),
            )
            .unwrap()
        };

        let listen_ips = create_listeners(
            runtime.clone(),
            &tn,
            sc.hosts,
            sc.update_interval,
            sc.wildcard_everything,
        );

        Self {
            runtime,
            tn: Arc::new(tn),
            resolvers: Arc::new(create_resolvers(listen_ips.clone())),
            listen_ips,
            update_interval: sc.update_interval,
        }
    }

    pub fn any_listen_ip(self) -> IpAddr {
        self.listen_ips
            .clone()
            .into_iter()
            .choose(&mut rand::thread_rng())
            .unwrap()
            .clone()
            .ip()
    }

    pub fn runtime(&self) -> Arc<Mutex<Runtime>> {
        self.runtime.clone()
    }

    pub fn network(&self) -> Arc<TestNetwork> {
        self.tn.clone()
    }

    pub fn resolvers(&self) -> Arc<Vec<Arc<Resolver>>> {
        self.resolvers.clone()
    }

    pub fn any_resolver(&self) -> Arc<Resolver> {
        self.resolvers()
            .choose(&mut rand::thread_rng())
            .to_owned()
            .unwrap()
            .clone()
    }

    pub fn member_record(&self) -> String {
        format!("zt-{}.domain.", self.network().identity().clone())
    }

    pub fn change_name(&self, name: &'static str) {
        let mut member = self
            .runtime()
            .lock()
            .unwrap()
            .block_on(
                zerotier_central_api::apis::network_member_api::get_network_member(
                    &self.network().central(),
                    &self.network().network.clone().id.unwrap(),
                    &self.network().identity(),
                ),
            )
            .unwrap();

        member.name = Some(name.to_string());

        self.runtime()
            .lock()
            .unwrap()
            .block_on(
                zerotier_central_api::apis::network_member_api::update_network_member(
                    &self.network().central(),
                    &self.network().network.clone().id.unwrap(),
                    &self.network().identity(),
                    member,
                ),
            )
            .unwrap();

        if self.update_interval.is_some() {
            sleep(self.update_interval.unwrap()); // wait for it to update
        }
    }

    pub fn test_network(&self) -> Arc<TestNetwork> {
        self.tn.clone()
    }
}

impl Lookup for Service {
    fn lookup_a(&self, record: String) -> Vec<Ipv4Addr> {
        self.any_resolver().lookup_a(record)
    }

    fn lookup_aaaa(&self, record: String) -> Vec<Ipv6Addr> {
        self.any_resolver().lookup_aaaa(record)
    }

    fn lookup_ptr(&self, record: String) -> Vec<String> {
        self.any_resolver().lookup_ptr(record)
    }
}
