use trust_dns_resolver::{
    config::{NameServerConfig, ResolverConfig, ResolverOpts},
    proto::rr::RecordType,
    IntoName, Resolver,
};

use crate::{
    authtoken_path, domain_or_default, get_listen_ip, init_authority, init_runtime,
    integration_tests::TestNetwork, parse_ip_from_cidr,
};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    str::FromStr,
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::runtime::Runtime;

struct Service {
    _runtime: Arc<Mutex<Runtime>>,
    tn: Arc<TestNetwork>,
    resolver: Arc<Resolver>,
    pub listen_ip: String,
    pub listen_cidr: String,
}

impl Service {
    fn new() -> Self {
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
            None,
            listen_cidr.clone(),
            listen_ip.clone(),
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
            _runtime: Arc::new(Mutex::new(runtime)),
            tn: Arc::new(tn),
            listen_ip,
            listen_cidr,
            resolver: Arc::new(resolver),
        }
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
    let service = Service::new();

    let record = format!("zt-{}.domain.", service.network().identity.clone());

    for _ in 0..100000 {
        assert_eq!(
            service.lookup_a(record.clone()).to_string(),
            service.listen_ip
        );
    }

    let ptr_record = IpAddr::from_str(&service.listen_ip)
        .unwrap()
        .into_name()
        .unwrap();

    for _ in 0..100000 {
        assert_eq!(
            service.lookup_ptr(ptr_record.to_string()),
            record.to_string()
        );
    }
}
