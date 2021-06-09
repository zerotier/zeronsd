use ipnetwork::IpNetwork;
use rand::{
    prelude::{IteratorRandom, SliceRandom},
    thread_rng,
};
use trust_dns_resolver::{
    config::{NameServerConfig, ResolverConfig, ResolverOpts},
    proto::rr::RecordType,
    IntoName, Name, Resolver,
};

use crate::{
    authority::new_ptr_authority,
    hosts::parse_hosts,
    integration_tests::{init_test_runtime, TestContext, TestNetwork},
    tests::HOSTS_DIR,
    utils::{
        authtoken_path, domain_or_default, get_listen_ips, init_authority, parse_ip_from_cidr,
    },
};
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    str::FromStr,
    sync::{mpsc::sync_channel, Arc, Mutex},
    thread::{self, sleep},
    time::Duration,
};

use tokio::runtime::Runtime;

use super::init_trust_dns_authority;

#[derive(Clone)]
struct Service {
    runtime: Arc<Mutex<Runtime>>,
    tn: Arc<TestNetwork>,
    resolvers: Arc<Vec<Arc<Resolver>>>,
    pub listen_ips: Vec<String>,
    pub listen_cidrs: Vec<String>,
}

pub(crate) trait Lookup {
    fn lookup_a(&self, record: String) -> Vec<Ipv4Addr>;
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

    fn lookup_ptr(&self, record: String) -> Vec<String> {
        self.lookup(record, RecordType::PTR)
            .unwrap()
            .record_iter()
            .map(|r| r.rdata().clone().into_ptr().unwrap().to_string())
            .collect()
    }
}

fn create_listeners(
    runtime: Arc<Mutex<Runtime>>,
    tn: &TestNetwork,
    hosts: Option<&str>,
    update_interval: Option<Duration>,
) -> (Vec<String>, Vec<String>) {
    let listen_cidrs = runtime
        .lock()
        .unwrap()
        .block_on(get_listen_ips(
            &authtoken_path(None),
            &tn.network.clone().id.unwrap(),
        ))
        .unwrap();

    let mut listen_ips = Vec::new();

    let (s, r) = sync_channel(listen_cidrs.len());

    let mut ipmap = HashMap::new();
    let mut authority_map = HashMap::new();
    let authority = init_trust_dns_authority(domain_or_default(None).unwrap());

    for cidr in listen_cidrs.clone() {
        let listen_ip = parse_ip_from_cidr(cidr.clone());
        listen_ips.push(listen_ip.clone());
        let cidr = IpNetwork::from_str(&cidr.clone()).unwrap();
        if !ipmap.contains_key(&listen_ip) {
            ipmap.insert(listen_ip, cidr);
        }

        if !authority_map.contains_key(&cidr) {
            let ptr_authority = new_ptr_authority(cidr).unwrap();

            let ztauthority = init_authority(
                ptr_authority.clone(),
                tn.token(),
                tn.network.clone().id.unwrap(),
                domain_or_default(None).unwrap(),
                match hosts {
                    Some(hosts) => Some(format!("{}/{}", HOSTS_DIR, hosts)),
                    None => None,
                },
                update_interval.unwrap_or(Duration::new(30, 0)),
                authority.clone(),
            )
            .unwrap();

            runtime
                .lock()
                .unwrap()
                .spawn(ztauthority.clone().find_members());
            authority_map.insert(cidr, ztauthority);
        }
    }

    for ip in listen_ips.clone() {
        let cidr = ipmap.get(&ip).unwrap();
        let authority = authority_map.get(cidr).unwrap();

        let sync = s.clone();

        let rt = &mut runtime.lock().unwrap();
        let server = crate::server::Server::new(authority.catalog(rt).unwrap());

        rt.spawn({
            sync.send(()).unwrap();
            drop(sync);
            server.listen(
                format!("{}:53", ip.clone()).to_owned(),
                Duration::new(0, 1000),
            )
        });
    }

    drop(s);

    loop {
        if r.recv().is_err() {
            break;
        }
    }

    sleep(Duration::new(2, 0)); // FIXME this sleep should not be necessary

    (listen_cidrs, listen_ips.clone())
}

fn create_resolvers(ips: Vec<String>) -> Vec<Arc<Resolver>> {
    let mut resolvers = Vec::new();

    for ip in ips {
        let mut resolver_config = ResolverConfig::new();
        resolver_config.add_search(domain_or_default(None).unwrap());
        resolver_config.add_name_server(NameServerConfig {
            socket_addr: SocketAddr::new(IpAddr::from_str(&ip).unwrap(), 53),
            protocol: trust_dns_resolver::config::Protocol::Udp,
            tls_dns_name: None,
            trust_nx_responses: true,
        });

        resolvers.push(Arc::new(
            trust_dns_resolver::Resolver::new(resolver_config, ResolverOpts::default()).unwrap(),
        ));
    }

    resolvers
}

impl Service {
    fn new(hosts: Option<&str>, update_interval: Option<Duration>, ips: Option<Vec<&str>>) -> Self {
        let runtime = init_test_runtime();

        let tn = if let Some(ips) = ips {
            TestNetwork::new_multi_ip(
                runtime.clone(),
                "basic-ipv4",
                &mut TestContext::default(),
                ips,
            )
            .unwrap()
        } else {
            TestNetwork::new(runtime.clone(), "basic-ipv4", &mut TestContext::default()).unwrap()
        };

        let (listen_cidrs, listen_ips) =
            create_listeners(runtime.clone(), &tn, hosts, update_interval);

        Self {
            runtime,
            tn: Arc::new(tn),
            listen_ips: listen_ips.clone(),
            listen_cidrs,
            resolvers: Arc::new(create_resolvers(listen_ips)),
        }
    }

    #[allow(dead_code)]
    pub fn any_listen_ip(&self) -> String {
        self.listen_ips
            .clone()
            .into_iter()
            .choose(&mut rand::thread_rng())
            .unwrap()
            .clone()
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

    pub fn lookup_a(&self, record: String) -> Vec<Ipv4Addr> {
        self.any_resolver().lookup_a(record)
    }

    pub fn lookup_ptr(self, record: String) -> Vec<String> {
        self.any_resolver().lookup_ptr(record)
    }
}

#[test]
#[ignore]
fn test_battery_single_domain() {
    let service = Service::new(
        None,
        None,
        Some(vec!["172.16.240.2", "172.16.240.3", "172.16.240.4"]),
    );

    let record = format!("zt-{}.domain.", service.network().identity().clone());

    eprintln!("Looking up {}", record);
    let mut listen_ips = service.listen_ips.clone();
    listen_ips.sort();

    for _ in 0..10000 {
        let mut ips = service
            .lookup_a(record.clone())
            .into_iter()
            .map(|i| i.to_string())
            .collect::<Vec<String>>();
        ips.sort();

        assert_eq!(ips, listen_ips);
    }

    let ptr_records: Vec<Name> = service
        .listen_ips
        .clone()
        .into_iter()
        .map(|ip| IpAddr::from_str(&ip).unwrap().into_name().unwrap())
        .collect();

    for ptr_record in ptr_records.clone() {
        eprintln!("Looking up {}", ptr_record);

        for _ in 0..10000 {
            let service = service.clone();
            assert_eq!(
                service.lookup_ptr(ptr_record.to_string()).first().unwrap(),
                &record.to_string()
            );
        }
    }

    eprintln!("Interleaved lookups of PTR and A records");

    for _ in 0..100000 {
        // randomly switch order
        if rand::random::<bool>() {
            assert_eq!(
                service
                    .lookup_a(record.clone())
                    .into_iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<String>>(),
                service.listen_ips,
            );

            assert_eq!(
                service
                    .clone()
                    .lookup_ptr(ptr_records.choose(&mut thread_rng()).unwrap().to_string())
                    .first()
                    .unwrap(),
                &record.to_string()
            );
        } else {
            assert_eq!(
                service
                    .clone()
                    .lookup_ptr(ptr_records.choose(&mut thread_rng()).unwrap().to_string())
                    .first()
                    .unwrap(),
                &record.to_string()
            );

            assert_eq!(
                service
                    .lookup_a(record.clone())
                    .into_iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<String>>(),
                service.listen_ips,
            );
        }
    }
}

#[test]
#[ignore]
fn test_battery_multi_domain_hosts_file() {
    let ips = vec!["172.16.240.2", "172.16.240.3", "172.16.240.4"];
    let service = Service::new(Some("basic"), None, Some(ips.clone()));

    let record = format!("zt-{}.domain.", service.network().identity().clone());

    eprintln!("Looking up random domains");

    let mut hosts_map = parse_hosts(
        Some(format!("{}/basic", HOSTS_DIR)),
        "domain.".into_name().unwrap(),
    )
    .unwrap();

    for ip in ips {
        hosts_map.insert(
            IpAddr::from_str(&ip).unwrap(),
            vec![record.clone().into_name().unwrap()],
        );
    }

    let mut hosts = hosts_map.values().flatten().collect::<Vec<&Name>>();
    for _ in 0..100000 {
        hosts.shuffle(&mut rand::thread_rng());
        let host = *hosts.first().unwrap();
        let ips = service.lookup_a(host.to_string());
        assert!(hosts_map
            .get(&IpAddr::from(*ips.first().unwrap()))
            .unwrap()
            .contains(host));
    }
}

#[test]
#[ignore]
fn test_battery_single_domain_named() {
    let update_interval = Duration::new(1, 0);
    let service = Service::new(
        None,
        Some(update_interval),
        Some(vec!["172.16.240.2", "172.16.240.3", "172.16.240.4"]),
    );
    let member_record = format!("zt-{}.domain.", service.network().identity().clone());

    let mut member = service
        .runtime()
        .lock()
        .unwrap()
        .block_on(
            zerotier_central_api::apis::network_member_api::get_network_member(
                &service.network().central(),
                &service.network().network.clone().id.unwrap(),
                &service.network().identity(),
            ),
        )
        .unwrap();

    member.name = Some("islay".to_string());

    service
        .runtime()
        .lock()
        .unwrap()
        .block_on(
            zerotier_central_api::apis::network_member_api::update_network_member(
                &service.network().central(),
                &service.network().network.clone().id.unwrap(),
                &service.network().identity(),
                member,
            ),
        )
        .unwrap();

    thread::sleep(update_interval); // wait for it to update

    let named_record = "islay.domain.".to_string();

    for record in vec![member_record, named_record.clone()] {
        eprintln!("Looking up {}", record);

        let mut listen_ips = service.listen_ips.clone();
        listen_ips.sort();

        for _ in 0..10000 {
            let mut ips = service
                .lookup_a(record.clone())
                .into_iter()
                .map(|i| i.to_string())
                .collect::<Vec<String>>();
            ips.sort();
            assert_eq!(ips, listen_ips.clone(),);
        }
    }

    let ptr_records: Vec<Name> = service
        .listen_ips
        .clone()
        .into_iter()
        .map(|ip| IpAddr::from_str(&ip).unwrap().into_name().unwrap())
        .collect();

    for ptr_record in ptr_records {
        eprintln!("Looking up {}", ptr_record);

        for _ in 0..10000 {
            let service = service.clone();
            assert_eq!(
                service.lookup_ptr(ptr_record.to_string()).first().unwrap(),
                &named_record.to_string()
            );
        }
    }
}
