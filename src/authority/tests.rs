use trust_dns_resolver::{IntoName, Name};

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

type SocketVec = Vec<SocketAddr>;

pub(crate) trait ToIPv4Vec {
    fn to_ipv4_vec(self) -> Vec<Ipv4Addr>;
}

pub(crate) trait ToIPv6Vec {
    fn to_ipv6_vec(self) -> Vec<Ipv6Addr>;
}

pub(crate) trait ToPTRVec {
    fn to_ptr_vec(self) -> Vec<Name>;
}

impl ToIPv4Vec for SocketVec {
    fn to_ipv4_vec(self) -> Vec<Ipv4Addr> {
        self.into_iter()
            .filter_map(|ip| match ip.ip() {
                IpAddr::V4(ip) => Some(ip),
                IpAddr::V6(_) => None,
            })
            .collect::<Vec<Ipv4Addr>>()
    }
}

impl ToIPv6Vec for SocketVec {
    fn to_ipv6_vec(self) -> Vec<Ipv6Addr> {
        self.into_iter()
            .filter_map(|ip| match ip.ip() {
                IpAddr::V4(_) => None,
                IpAddr::V6(ip) => Some(ip),
            })
            .collect::<Vec<Ipv6Addr>>()
    }
}

impl ToPTRVec for SocketVec {
    fn to_ptr_vec(self) -> Vec<Name> {
        self.into_iter()
            .map(|ip| ip.ip().into_name().unwrap())
            .collect::<Vec<Name>>()
    }
}

mod sixplane {
    use std::{net::IpAddr, path::Path, str::FromStr, time::Duration};

    use log::info;
    use rand::prelude::SliceRandom;
    use trust_dns_resolver::{IntoName, Name};

    use crate::{
        addresses::Calculator,
        authority::{
            service::{HostsType, Lookup, Service, ServiceConfig},
            tests::ToIPv6Vec,
        },
        hosts::parse_hosts,
        tests::HOSTS_DIR,
        utils::init_logger,
    };

    #[test]
    fn test_battery_single_domain() {
        init_logger();
        let service = Service::new(ServiceConfig::default().network_filename("6plane-only"));

        let record = service.member_record();

        info!("Looking up {}", record);
        let mut listen_ips = service.listen_ips.clone();
        listen_ips.sort();

        for _ in 0..10000 {
            let mut ips = service.lookup_aaaa(record.clone());
            ips.sort();

            assert_eq!(ips.sort(), listen_ips.clone().to_ipv6_vec().sort());
        }
    }

    #[test]
    fn test_battery_single_domain_named() {
        init_logger();
        let update_interval = Duration::new(20, 0);
        let service = Service::new(
            ServiceConfig::default()
                .update_interval(Some(update_interval))
                .network_filename("6plane-only"),
        );
        let member_record = service.member_record();

        service.change_name("islay");

        let named_record = "islay.domain.".to_string();

        for record in vec![member_record, named_record.clone()] {
            info!("Looking up {}", record);

            let mut listen_ips = service.listen_ips.clone();
            listen_ips.sort();

            for _ in 0..10000 {
                let mut ips = service.lookup_aaaa(record.clone());
                ips.sort();
                assert_eq!(ips, listen_ips.clone().to_ipv6_vec());
            }
        }
    }

    #[test]
    fn test_battery_multi_domain_hosts_file() {
        init_logger();
        let service = Service::new(
            ServiceConfig::default()
                .hosts(HostsType::Fixture("basic-ipv6"))
                .network_filename("6plane-only"),
        );

        let record = service.member_record();

        info!("Looking up random domains");

        let mut hosts_map = parse_hosts(
            Some(Path::new(&format!("{}/basic-ipv6", HOSTS_DIR)).to_path_buf()),
            "domain.".into_name().unwrap(),
        )
        .unwrap();

        let ip = service.test_network().member().sixplane().unwrap().ip();
        hosts_map.insert(ip, vec![record.clone().into_name().unwrap()]);

        let mut hosts = hosts_map.values().flatten().collect::<Vec<&Name>>();
        for _ in 0..10000 {
            hosts.shuffle(&mut rand::thread_rng());
            let host = *hosts.first().unwrap();
            let ip = service.lookup_aaaa(host.to_string());
            assert!(hosts_map
                .get(&IpAddr::V6(*ip.first().unwrap()))
                .unwrap()
                .contains(host));
        }
    }

    #[test]
    fn test_wildcard_central() {
        init_logger();
        let service = Service::new(
            ServiceConfig::default()
                .update_interval(Some(Duration::new(20, 0)))
                .network_filename("6plane-only")
                .wildcard_everything(true),
        );

        let member_record = service.member_record();
        let named_record = Name::from_str("islay.domain.").unwrap();

        service.change_name("islay");

        assert_eq!(
            service
                .lookup_aaaa(named_record.to_string())
                .first()
                .unwrap(),
            &service.clone().any_listen_ip()
        );

        assert_eq!(
            service
                .lookup_aaaa(member_record.to_string())
                .first()
                .unwrap(),
            &service.clone().any_listen_ip()
        );

        for host in vec!["one", "ten", "zt-foo", "another-record"] {
            for rec in vec![named_record.to_string(), member_record.clone()] {
                let lookup = Name::from_str(&host)
                    .unwrap()
                    .append_domain(&Name::from_str(&rec).unwrap())
                    .to_string();
                assert_eq!(
                    service.lookup_aaaa(lookup).first().unwrap(),
                    &service.clone().any_listen_ip()
                );
            }
        }
    }
}

mod rfc4193 {
    use std::{net::IpAddr, path::Path, str::FromStr, time::Duration};

    use log::info;
    use rand::{prelude::SliceRandom, thread_rng};
    use trust_dns_resolver::{IntoName, Name};

    use crate::{
        addresses::Calculator,
        authority::{
            service::{HostsType, Lookup, Service, ServiceConfig},
            tests::{ToIPv6Vec, ToPTRVec},
        },
        hosts::parse_hosts,
        tests::HOSTS_DIR,
        utils::init_logger,
    };

    #[test]
    fn test_battery_single_domain() {
        init_logger();
        let service = Service::new(ServiceConfig::default().network_filename("rfc4193-only"));

        let record = service.member_record();

        info!("Looking up {}", record);
        let mut listen_ips = service.listen_ips.clone();
        listen_ips.sort();

        for _ in 0..10000 {
            let mut ips = service.lookup_aaaa(record.clone());
            ips.sort();

            assert_eq!(ips.sort(), listen_ips.clone().to_ipv6_vec().sort());
        }

        let ptr_records: Vec<Name> = service
            .listen_ips
            .clone()
            .into_iter()
            .map(|ip| ip.ip().into_name().unwrap())
            .collect();

        for ptr_record in ptr_records.clone() {
            info!("Looking up {}", ptr_record);

            for _ in 0..10000 {
                let service = service.clone();
                assert_eq!(
                    service.lookup_ptr(ptr_record.to_string()).first().unwrap(),
                    &record.to_string()
                );
            }
        }

        info!("Interleaved lookups of PTR and AAAA records");

        for _ in 0..10000 {
            // randomly switch order
            if rand::random::<bool>() {
                assert_eq!(
                    service.lookup_aaaa(record.clone()).sort(),
                    listen_ips.clone().to_ipv6_vec().sort()
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
                    service.lookup_aaaa(record.clone()).sort(),
                    listen_ips.clone().to_ipv6_vec().sort()
                );
            }
        }
    }

    #[test]
    fn test_battery_single_domain_named() {
        init_logger();
        let update_interval = Duration::new(20, 0);
        let service = Service::new(
            ServiceConfig::default()
                .update_interval(Some(update_interval))
                .network_filename("rfc4193-only"),
        );
        let member_record = service.member_record();

        service.change_name("islay");

        let named_record = "islay.domain.".to_string();

        for record in vec![member_record, named_record.clone()] {
            info!("Looking up {}", record);

            let mut listen_ips = service.listen_ips.clone();
            listen_ips.sort();

            for _ in 0..10000 {
                let mut ips = service.lookup_aaaa(record.clone());
                ips.sort();
                assert_eq!(ips, listen_ips.clone().to_ipv6_vec());
            }
        }

        let ptr_records: Vec<Name> = service.listen_ips.clone().to_ptr_vec();

        for ptr_record in ptr_records {
            info!("Looking up {}", ptr_record);

            for _ in 0..10000 {
                let service = service.clone();
                assert_eq!(
                    service.lookup_ptr(ptr_record.to_string()).first().unwrap(),
                    &named_record.to_string()
                );
            }
        }
    }

    #[test]
    fn test_battery_multi_domain_hosts_file() {
        init_logger();
        let service = Service::new(
            ServiceConfig::default()
                .hosts(HostsType::Fixture("basic-ipv6"))
                .network_filename("rfc4193-only"),
        );

        let record = service.member_record();

        info!("Looking up random domains");

        let mut hosts_map = parse_hosts(
            Some(Path::new(&format!("{}/basic-ipv6", HOSTS_DIR)).to_path_buf()),
            "domain.".into_name().unwrap(),
        )
        .unwrap();

        let ip = service.test_network().member().rfc4193().unwrap().ip();
        hosts_map.insert(ip, vec![record.clone().into_name().unwrap()]);

        let mut hosts = hosts_map.values().flatten().collect::<Vec<&Name>>();
        for _ in 0..10000 {
            hosts.shuffle(&mut rand::thread_rng());
            let host = *hosts.first().unwrap();
            let ip = service.lookup_aaaa(host.to_string());
            assert!(hosts_map
                .get(&IpAddr::V6(*ip.first().unwrap()))
                .unwrap()
                .contains(host));
        }
    }

    #[test]
    fn test_wildcard_central() {
        init_logger();
        let service = Service::new(
            ServiceConfig::default()
                .update_interval(Some(Duration::new(20, 0)))
                .network_filename("rfc4193-only")
                .wildcard_everything(true),
        );

        let member_record = service.member_record();
        let named_record = Name::from_str("islay.domain.").unwrap();

        service.change_name("islay");

        assert_eq!(
            service
                .lookup_aaaa(named_record.to_string())
                .first()
                .unwrap(),
            &service.clone().any_listen_ip()
        );

        assert_eq!(
            service
                .lookup_aaaa(member_record.to_string())
                .first()
                .unwrap(),
            &service.clone().any_listen_ip()
        );

        for host in vec!["one", "ten", "zt-foo", "another-record"] {
            for rec in vec![named_record.to_string(), member_record.clone()] {
                let lookup = Name::from_str(&host)
                    .unwrap()
                    .append_domain(&Name::from_str(&rec).unwrap())
                    .to_string();
                assert_eq!(
                    service.lookup_aaaa(lookup).first().unwrap(),
                    &service.clone().any_listen_ip()
                );
            }
        }
    }
}

mod ipv4 {
    use std::{str::FromStr, time::Duration};

    use log::info;
    use rand::{prelude::SliceRandom, thread_rng};
    use trust_dns_resolver::{IntoName, Name};

    use crate::{
        authority::{
            service::{Lookup, Service, ServiceConfig},
            tests::{ToIPv4Vec, ToPTRVec},
        },
        utils::init_logger,
    };

    #[test]
    fn test_wildcard_central() {
        init_logger();
        let service = Service::new(
            ServiceConfig::default()
                .update_interval(Some(Duration::new(20, 0)))
                .wildcard_everything(true),
        );

        let member_record = service.member_record();
        let named_record = Name::from_str("islay.domain.").unwrap();

        service.change_name("islay");

        assert_eq!(
            service.lookup_a(named_record.to_string()).first().unwrap(),
            &service.clone().any_listen_ip()
        );

        assert_eq!(
            service.lookup_a(member_record.to_string()).first().unwrap(),
            &service.clone().any_listen_ip()
        );

        for host in vec!["one", "ten", "zt-foo", "another-record"] {
            for rec in vec![named_record.to_string(), member_record.clone()] {
                let lookup = Name::from_str(&host)
                    .unwrap()
                    .append_domain(&Name::from_str(&rec).unwrap())
                    .to_string();
                assert_eq!(
                    service.lookup_a(lookup).first().unwrap(),
                    &service.clone().any_listen_ip()
                );
            }
        }
    }

    #[test]
    fn test_battery_single_domain() {
        init_logger();
        let service = Service::new(ServiceConfig::default().ips(Some(vec![
            "172.16.240.2",
            "172.16.240.3",
            "172.16.240.4",
        ])));

        let record = service.member_record();

        info!("Looking up {}", record);
        let mut listen_ips = service.listen_ips.clone();
        listen_ips.sort();

        for _ in 0..10000 {
            let mut ips = service.lookup_a(record.clone());
            ips.sort();

            assert_eq!(ips.sort(), listen_ips.clone().to_ipv4_vec().sort());
        }

        let ptr_records: Vec<Name> = service
            .listen_ips
            .clone()
            .into_iter()
            .map(|ip| ip.ip().into_name().unwrap())
            .collect();

        for ptr_record in ptr_records.clone() {
            info!("Looking up {}", ptr_record);

            for _ in 0..10000 {
                let service = service.clone();
                assert_eq!(
                    service.lookup_ptr(ptr_record.to_string()).first().unwrap(),
                    &record.to_string()
                );
            }
        }

        info!("Interleaved lookups of PTR and A records");

        for _ in 0..10000 {
            // randomly switch order
            if rand::random::<bool>() {
                assert_eq!(
                    service.lookup_a(record.clone()).sort(),
                    listen_ips.clone().to_ipv4_vec().sort()
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
                    service.lookup_a(record.clone()).sort(),
                    listen_ips.clone().to_ipv4_vec().sort()
                );
            }
        }
    }

    #[test]
    fn test_battery_single_domain_named() {
        init_logger();
        let update_interval = Duration::new(20, 0);
        let service = Service::new(
            ServiceConfig::default()
                .update_interval(Some(update_interval))
                .ips(Some(vec!["172.16.240.2", "172.16.240.3", "172.16.240.4"])),
        );
        let member_record = service.member_record();

        service.change_name("islay");

        let named_record = "islay.domain.".to_string();

        for record in vec![member_record, named_record.clone()] {
            info!("Looking up {}", record);

            let mut listen_ips = service.listen_ips.clone();
            listen_ips.sort();

            for _ in 0..10000 {
                let mut ips = service.lookup_a(record.clone());
                ips.sort();
                assert_eq!(ips, listen_ips.clone().to_ipv4_vec());
            }
        }

        let ptr_records: Vec<Name> = service.listen_ips.clone().to_ptr_vec();

        for ptr_record in ptr_records {
            info!("Looking up {}", ptr_record);

            for _ in 0..10000 {
                let service = service.clone();
                assert_eq!(
                    service.lookup_ptr(ptr_record.to_string()).first().unwrap(),
                    &named_record.to_string()
                );
            }
        }
    }
}

mod all {
    use log::info;
    use rand::prelude::SliceRandom;
    use trust_dns_resolver::{IntoName, Name};

    use crate::{
        authority::service::{HostsType, Lookup, Service, ServiceConfig},
        hosts::parse_hosts,
        tests::HOSTS_DIR,
        utils::init_logger,
    };

    use std::{
        net::{IpAddr, Ipv4Addr, Ipv6Addr},
        path::Path,
        str::FromStr,
        thread::sleep,
        time::Duration,
    };

    #[test]
    fn test_battery_multi_domain_hosts_file() {
        init_logger();
        let ips = vec!["172.16.240.2", "172.16.240.3", "172.16.240.4"];
        let service = Service::new(
            ServiceConfig::default()
                .hosts(HostsType::Fixture("basic"))
                .ips(Some(ips.clone())),
        );

        let record = service.member_record();

        info!("Looking up random domains");

        let mut hosts_map = parse_hosts(
            Some(Path::new(&format!("{}/basic", HOSTS_DIR)).to_path_buf()),
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
        for _ in 0..10000 {
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
    fn test_hosts_file_reloading() {
        init_logger();
        let hosts_path = "/tmp/zeronsd-test-hosts";
        std::fs::write(hosts_path, "127.0.0.2 islay\n::2 islay\n").unwrap();
        let service = Service::new(
            ServiceConfig::default()
                .hosts(HostsType::Path(hosts_path))
                .update_interval(Some(Duration::new(20, 0))),
        );

        assert_eq!(
            service
                .lookup_a("islay.domain.".to_string())
                .first()
                .unwrap(),
            &Ipv4Addr::from_str("127.0.0.2").unwrap()
        );

        assert_eq!(
            service
                .lookup_aaaa("islay.domain.".to_string())
                .first()
                .unwrap(),
            &Ipv6Addr::from_str("::2").unwrap()
        );

        std::fs::write(hosts_path, "127.0.0.3 islay\n::3 islay\n").unwrap();
        sleep(Duration::new(30, 0)); // wait for bg update

        assert_eq!(
            service
                .lookup_a("islay.domain.".to_string())
                .first()
                .unwrap(),
            &Ipv4Addr::from_str("127.0.0.3").unwrap()
        );

        assert_eq!(
            service
                .lookup_aaaa("islay.domain.".to_string())
                .first()
                .unwrap(),
            &Ipv6Addr::from_str("::3").unwrap()
        );
    }
}
