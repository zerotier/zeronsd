use rand::prelude::SliceRandom;
use trust_dns_resolver::{
    config::{NameServerConfig, ResolverConfig, ResolverOpts},
    proto::rr::RecordType,
    IntoName, Name, Resolver,
};

use crate::{
    hosts::parse_hosts,
    integration_tests::TestNetwork,
    tests::HOSTS_DIR,
    utils::{
        authtoken_path, domain_or_default, get_listen_ip, init_authority, init_runtime,
        parse_ip_from_cidr,
    },
};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    str::FromStr,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use tokio::runtime::Runtime;

struct Service {
    runtime: Arc<Mutex<Runtime>>,
    tn: Arc<TestNetwork>,
    resolver: Arc<Resolver>,
    pub listen_ip: String,
    pub listen_cidr: String,
}

impl Service {
    fn new(hosts: Option<&str>, update_interval: Option<Duration>) -> Self {
        let mut runtime = init_runtime();
        let tn = TestNetwork::new("basic-ipv4").unwrap();

        let listen_cidr = runtime
            .block_on(get_listen_ip(
                &authtoken_path(None),
                &tn.network.clone().id.unwrap(),
            ))
            .unwrap();

        let listen_ip = parse_ip_from_cidr(listen_cidr.clone());

        let server = init_authority(
            &mut runtime,
            tn.central_token.clone(),
            tn.network.clone().id.unwrap(),
            domain_or_default(None).unwrap(),
            match hosts {
                Some(hosts) => Some(format!("{}/{}", HOSTS_DIR, hosts)),
                None => None,
            },
            listen_cidr.clone(),
            listen_ip.clone(),
            update_interval.unwrap_or(Duration::new(30, 0)),
        )
        .unwrap();

        runtime.spawn(server.listen(
            format!("{}:53", listen_ip.clone()).to_owned(),
            Duration::new(0, 1000),
        ));

        let mut resolver_config = ResolverConfig::new();
        resolver_config.add_search(domain_or_default(None).unwrap());
        resolver_config.add_name_server(NameServerConfig {
            socket_addr: SocketAddr::new(IpAddr::from_str(&listen_ip).unwrap(), 53),
            protocol: trust_dns_resolver::config::Protocol::Udp,
            tls_dns_name: None,
            trust_nx_responses: true,
        });

        let resolver =
            trust_dns_resolver::Resolver::new(resolver_config, ResolverOpts::default()).unwrap();

        Self {
            runtime: Arc::new(Mutex::new(runtime)),
            tn: Arc::new(tn),
            listen_ip,
            listen_cidr,
            resolver: Arc::new(resolver),
        }
    }

    pub fn runtime(&self) -> Arc<Mutex<Runtime>> {
        self.runtime.clone()
    }

    pub fn network(&self) -> Arc<TestNetwork> {
        self.tn.clone()
    }

    pub fn resolver(&self) -> Arc<Resolver> {
        self.resolver.clone()
    }

    pub fn lookup_a(&self, record: String) -> Ipv4Addr {
        self.resolver()
            .lookup(record, RecordType::A)
            .unwrap()
            .record_iter()
            .nth(0)
            .unwrap()
            .rdata()
            .clone()
            .into_a()
            .unwrap()
    }

    pub fn lookup_ptr(&self, record: String) -> String {
        self.resolver()
            .lookup(record, RecordType::PTR)
            .unwrap()
            .record_iter()
            .nth(0)
            .unwrap()
            .rdata()
            .clone()
            .into_ptr()
            .unwrap()
            .to_string()
    }
}

#[test]
#[ignore]
fn test_battery_single_domain() {
    let service = Service::new(None, None);

    let record = format!("zt-{}.domain.", service.network().identity.clone());

    eprintln!("Looking up {}", record);

    for _ in 0..10000 {
        assert_eq!(
            service.lookup_a(record.clone()).to_string(),
            service.listen_ip
        );
    }

    let ptr_record = IpAddr::from_str(&service.listen_ip)
        .unwrap()
        .into_name()
        .unwrap();

    eprintln!("Looking up {}", ptr_record);

    for _ in 0..10000 {
        assert_eq!(
            service.lookup_ptr(ptr_record.to_string()),
            record.to_string()
        );
    }

    eprintln!("Interleaved lookups of PTR and A records");

    for _ in 0..100000 {
        // randomly switch order
        if rand::random::<bool>() {
            assert_eq!(
                service.lookup_a(record.clone()).to_string(),
                service.listen_ip
            );

            assert_eq!(
                service.lookup_ptr(ptr_record.to_string()),
                record.to_string()
            );
        } else {
            assert_eq!(
                service.lookup_ptr(ptr_record.to_string()),
                record.to_string()
            );

            assert_eq!(
                service.lookup_a(record.clone()).to_string(),
                service.listen_ip
            );
        }
    }
}

#[test]
#[ignore]
fn test_battery_multi_domain_hosts_file() {
    let service = Service::new(Some("basic"), None);

    let record = format!("zt-{}.domain.", service.network().identity.clone());

    eprintln!("Looking up random domains");

    let mut hosts_map = parse_hosts(
        Some(format!("{}/basic", HOSTS_DIR)),
        "domain.".into_name().unwrap(),
    )
    .unwrap();

    hosts_map.insert(
        IpAddr::from_str(&service.listen_ip).unwrap(),
        vec![record.into_name().unwrap()],
    );

    let mut hosts = hosts_map.values().flatten().collect::<Vec<&Name>>();
    for _ in 0..100000 {
        hosts.shuffle(&mut rand::thread_rng());
        let host = hosts.first().unwrap();
        let ip = service.lookup_a(host.to_string());
        assert!(hosts_map.get(&ip.into()).unwrap().contains(host));
    }
}

#[test]
#[ignore]
fn test_battery_single_domain_named() {
    let update_interval = Duration::new(1, 0);
    let service = Service::new(None, Some(update_interval));
    let member_record = format!("zt-{}.domain.", service.network().identity.clone());

    let mut member = service
        .runtime()
        .lock()
        .unwrap()
        .block_on(
            zerotier_central_api::apis::network_member_api::get_network_member(
                &service.network().central,
                &service.network().network.clone().id.unwrap(),
                &service.network().identity,
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
                &service.network().central,
                &service.network().network.clone().id.unwrap(),
                &service.network().identity,
                member,
            ),
        )
        .unwrap();

    thread::sleep(update_interval); // wait for it to update

    let named_record = "islay.domain.".to_string();

    for record in vec![member_record, named_record.clone()] {
        eprintln!("Looking up {}", record);

        for _ in 0..10000 {
            assert_eq!(
                service.lookup_a(record.clone()).to_string(),
                service.listen_ip
            );
        }
    }

    let ptr_record = IpAddr::from_str(&service.listen_ip)
        .unwrap()
        .into_name()
        .unwrap();

    eprintln!("Looking up {}", ptr_record);

    for _ in 0..10000 {
        assert_eq!(
            service.lookup_ptr(ptr_record.to_string()),
            named_record.to_string(),
        );
    }
}
