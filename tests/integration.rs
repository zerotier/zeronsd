use zeronsd::utils::init_logger;

mod service;

#[ctor::ctor]
fn init() {
    init_logger(Some(tracing::Level::ERROR));
}

mod sixplane {
    use std::{net::IpAddr, path::Path, str::FromStr, time::Duration};

    use rand::prelude::SliceRandom;
    use tracing::info;
    use trust_dns_resolver::{IntoName, Name};

    use crate::service::{
        resolver::Lookup, to_ip::ToIPv6Vec, utils::HostsType, Service, ServiceConfig,
    };
    use zeronsd::{addresses::Calculator, hosts::parse_hosts};

    #[tokio::test(flavor = "multi_thread")]
    async fn test_battery_single_domain() {
        let service = Service::new(ServiceConfig::default().network_filename("6plane-only")).await;

        let record = service.member_record();

        info!("Looking up {}", record);
        let mut listen_ips = service.listen_ips.clone();
        listen_ips.sort();

        for _ in 0..1000 {
            let mut ips = service.lookup_aaaa(record.clone()).await;
            ips.sort();

            assert_eq!(ips, listen_ips.clone().to_ipv6_vec());
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_battery_single_domain_named() {
        let update_interval = Duration::new(2, 0);
        let service = Service::new(
            ServiceConfig::default()
                .update_interval(Some(update_interval))
                .network_filename("6plane-only"),
        )
        .await;
        let member_record = service.member_record();

        service.change_name("islay").await;

        let named_record = "islay.home.arpa.".to_string();

        for record in vec![member_record, named_record.clone()] {
            info!("Looking up {}", record);

            let mut listen_ips = service.listen_ips.clone();
            listen_ips.sort();

            for _ in 0..1000 {
                let mut ips = service.lookup_aaaa(record.clone()).await;
                ips.sort();
                assert_eq!(ips, listen_ips.clone().to_ipv6_vec());
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_battery_multi_domain_hosts_file() {
        let service = Service::new(
            ServiceConfig::default()
                .hosts(HostsType::Fixture("basic-ipv6"))
                .network_filename("6plane-only"),
        )
        .await;

        let record = service.member_record();

        info!("Looking up random domains");

        let mut hosts_map = parse_hosts(
            Some(
                Path::new(&format!("{}/basic-ipv6", zeronsd::utils::TEST_HOSTS_DIR)).to_path_buf(),
            ),
            "home.arpa.".into_name().unwrap(),
        )
        .unwrap();

        let ip = service.test_network().member().sixplane().unwrap().ip();
        hosts_map.insert(ip, vec![record.clone().into_name().unwrap()]);

        let mut hosts = hosts_map.values().flatten().collect::<Vec<&Name>>();
        for _ in 0..1000 {
            hosts.shuffle(&mut rand::thread_rng());
            let host = *hosts.first().unwrap();
            let ip = service.lookup_aaaa(host.to_string()).await;
            assert!(hosts_map
                .get(&IpAddr::V6(*ip.first().unwrap()))
                .unwrap()
                .contains(host));
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_wildcard_central() {
        let service = Service::new(
            ServiceConfig::default()
                .update_interval(Some(Duration::new(5, 0)))
                .network_filename("6plane-only")
                .wildcard_everything(true),
        )
        .await;

        let member_record = service.member_record();
        let named_record = Name::from_str("islay.home.arpa.").unwrap();

        service.change_name("islay").await;

        assert_eq!(
            service
                .lookup_aaaa(named_record.to_string())
                .await
                .first()
                .unwrap(),
            &service.clone().any_listen_ip()
        );

        assert_eq!(
            service
                .lookup_aaaa(member_record.to_string())
                .await
                .first()
                .unwrap(),
            &service.clone().any_listen_ip()
        );

        for host in vec!["one", "ten", "zt-foo", "another-record"] {
            for rec in vec![named_record.to_string(), member_record.clone()] {
                let lookup = Name::from_str(&host)
                    .unwrap()
                    .append_domain(&Name::from_str(&rec).unwrap())
                    .unwrap()
                    .to_string();
                assert_eq!(
                    service.lookup_aaaa(lookup).await.first().unwrap(),
                    &service.clone().any_listen_ip()
                );
            }
        }
    }
}

mod rfc4193 {
    use std::{net::IpAddr, path::Path, str::FromStr, time::Duration};

    use rand::{prelude::SliceRandom, thread_rng};
    use tracing::info;
    use trust_dns_resolver::{IntoName, Name};
    use zeronsd::{addresses::Calculator, hosts::parse_hosts};

    use crate::service::{
        resolver::Lookup,
        to_ip::{ToIPv6Vec, ToPTRVec},
        utils::HostsType,
        Service, ServiceConfig,
    };

    #[tokio::test(flavor = "multi_thread")]
    async fn test_battery_single_domain() {
        let service = Service::new(ServiceConfig::default().network_filename("rfc4193-only")).await;

        let record = service.member_record();

        info!("Looking up {}", record);
        let mut listen_ips = service.listen_ips.clone();
        listen_ips.sort();

        for _ in 0..1000 {
            let mut ips = service.lookup_aaaa(record.clone()).await;
            ips.sort();

            assert_eq!(ips, listen_ips.clone().to_ipv6_vec());
        }

        let ptr_records: Vec<String> = service
            .listen_ips
            .clone()
            .into_iter()
            .map(|ip| ip.ip().to_string())
            .collect();

        for ptr_record in ptr_records.clone() {
            info!("Looking up {}", ptr_record);

            for _ in 0..1000 {
                let service = service.clone();
                assert_eq!(
                    service
                        .lookup_ptr(ptr_record.clone())
                        .await
                        .first()
                        .unwrap(),
                    &record.to_string()
                );
            }
        }

        info!("Interleaved lookups of PTR and AAAA records");

        for _ in 0..1000 {
            // randomly switch order
            if rand::random::<bool>() {
                let mut ips = service.lookup_aaaa(record.clone()).await;
                ips.sort();

                assert_eq!(ips, listen_ips.clone().to_ipv6_vec(),);

                assert_eq!(
                    service
                        .clone()
                        .lookup_ptr(ptr_records.choose(&mut thread_rng()).unwrap().to_string())
                        .await
                        .first()
                        .unwrap(),
                    &record.to_string()
                );
            } else {
                assert_eq!(
                    service
                        .clone()
                        .lookup_ptr(ptr_records.choose(&mut thread_rng()).unwrap().to_string())
                        .await
                        .first()
                        .unwrap(),
                    &record.to_string()
                );

                let mut ips = service.lookup_aaaa(record.clone()).await;
                ips.sort();
                assert_eq!(ips, listen_ips.clone().to_ipv6_vec());
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_battery_single_domain_named() {
        let update_interval = Duration::new(2, 0);
        let service = Service::new(
            ServiceConfig::default()
                .update_interval(Some(update_interval))
                .network_filename("rfc4193-only"),
        )
        .await;

        let member_record = service.member_record();

        service.change_name("islay").await;

        let named_record = "islay.home.arpa.".to_string();

        for record in vec![member_record, named_record.clone()] {
            info!("Looking up {}", record);

            let mut listen_ips = service.listen_ips.clone();
            listen_ips.sort();

            for _ in 0..1000 {
                let mut ips = service.lookup_aaaa(record.clone()).await;
                ips.sort();
                assert_eq!(ips, listen_ips.clone().to_ipv6_vec());
            }
        }

        let ptr_records: Vec<String> = service.listen_ips.clone().to_ptr_vec();

        for ptr_record in ptr_records {
            info!("Looking up {}", ptr_record);

            for _ in 0..1000 {
                let service = service.clone();
                assert_eq!(
                    service
                        .lookup_ptr(ptr_record.clone())
                        .await
                        .first()
                        .unwrap(),
                    &named_record.to_string()
                );
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_battery_multi_domain_hosts_file() {
        let service = Service::new(
            ServiceConfig::default()
                .hosts(HostsType::Fixture("basic-ipv6"))
                .network_filename("rfc4193-only"),
        )
        .await;

        let record = service.member_record();

        info!("Looking up random domains");

        let mut hosts_map = parse_hosts(
            Some(
                Path::new(&format!("{}/basic-ipv6", zeronsd::utils::TEST_HOSTS_DIR)).to_path_buf(),
            ),
            "home.arpa.".into_name().unwrap(),
        )
        .unwrap();

        let ip = service.test_network().member().rfc4193().unwrap().ip();
        hosts_map.insert(ip, vec![record.clone().into_name().unwrap()]);

        let mut hosts = hosts_map.values().flatten().collect::<Vec<&Name>>();
        for _ in 0..1000 {
            hosts.shuffle(&mut rand::thread_rng());
            let host = *hosts.first().unwrap();
            let ip = service.lookup_aaaa(host.to_string()).await;
            assert!(hosts_map
                .get(&IpAddr::V6(*ip.first().unwrap()))
                .unwrap()
                .contains(host));
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_wildcard_central() {
        let service = Service::new(
            ServiceConfig::default()
                .update_interval(Some(Duration::new(5, 0)))
                .network_filename("rfc4193-only")
                .wildcard_everything(true),
        )
        .await;

        let member_record = service.member_record();
        let named_record = Name::from_str("islay.home.arpa.").unwrap();

        service.change_name("islay").await;

        assert_eq!(
            service
                .lookup_aaaa(named_record.to_string())
                .await
                .first()
                .unwrap(),
            &service.clone().any_listen_ip()
        );

        assert_eq!(
            service
                .lookup_aaaa(member_record.to_string())
                .await
                .first()
                .unwrap(),
            &service.clone().any_listen_ip()
        );

        for host in vec!["one", "ten", "zt-foo", "another-record"] {
            for rec in vec![named_record.to_string(), member_record.clone()] {
                let lookup = Name::from_str(&host)
                    .unwrap()
                    .append_domain(&Name::from_str(&rec).unwrap())
                    .unwrap()
                    .to_string();
                assert_eq!(
                    service.lookup_aaaa(lookup).await.first().unwrap(),
                    &service.clone().any_listen_ip()
                );
            }
        }
    }
}

mod ipv4 {
    use std::time::Duration;

    use std::str::FromStr;
    use tracing::info;
    use trust_dns_resolver::Name;

    use crate::service::{
        resolver::Lookup,
        to_ip::{ToIPv4Vec, ToPTRVec},
        Service, ServiceConfig,
    };

    #[tokio::test(flavor = "multi_thread")]
    async fn test_wildcard_central() {
        let service = Service::new(
            ServiceConfig::default()
                .update_interval(Some(Duration::new(5, 0)))
                .wildcard_everything(true),
        )
        .await;

        let member_record = service.member_record();
        let named_record = Name::from_str("islay.home.arpa.").unwrap();

        service.change_name("islay").await;

        assert_eq!(
            service
                .lookup_a(named_record.to_string())
                .await
                .first()
                .unwrap(),
            &service.clone().any_listen_ip()
        );

        assert_eq!(
            service
                .lookup_a(member_record.to_string())
                .await
                .first()
                .unwrap(),
            &service.clone().any_listen_ip()
        );

        for host in vec!["one", "ten", "zt-foo", "another-record"] {
            for rec in vec![named_record.to_string(), member_record.clone()] {
                let lookup = Name::from_str(&host)
                    .unwrap()
                    .append_domain(&Name::from_str(&rec).unwrap())
                    .unwrap()
                    .to_string();
                assert_eq!(
                    service.lookup_a(lookup).await.first().unwrap(),
                    &service.clone().any_listen_ip()
                );
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_battery_single_domain() {
        use rand::{seq::SliceRandom, thread_rng};

        let service = Service::new(ServiceConfig::default().ips(Some(vec![
            "172.16.240.2",
            "172.16.240.3",
            "172.16.240.4",
        ])))
        .await;

        let record = service.member_record();

        info!("Looking up {}", record);
        let mut listen_ips = service.listen_ips.clone();
        listen_ips.sort();

        for _ in 0..1000 {
            let mut ips = service.lookup_a(record.clone()).await;
            ips.sort();

            assert_eq!(ips, listen_ips.clone().to_ipv4_vec());
        }

        let ptr_records: Vec<String> = service
            .listen_ips
            .clone()
            .into_iter()
            .map(|ip| ip.ip().to_string())
            .collect();

        for ptr_record in ptr_records.clone() {
            info!("Looking up {}", ptr_record);

            for _ in 0..1000 {
                let service = service.clone();
                assert_eq!(
                    service
                        .lookup_ptr(ptr_record.clone())
                        .await
                        .first()
                        .unwrap(),
                    &record.to_string()
                );
            }
        }

        info!("Interleaved lookups of PTR and A records");

        for _ in 0..1000 {
            // randomly switch order
            if rand::random::<bool>() {
                let mut ips = service.lookup_a(record.clone()).await;
                ips.sort();
                assert_eq!(ips, listen_ips.clone().to_ipv4_vec());

                assert_eq!(
                    service
                        .clone()
                        .lookup_ptr(ptr_records.choose(&mut thread_rng()).unwrap().to_string())
                        .await
                        .first()
                        .unwrap(),
                    &record.to_string()
                );
            } else {
                assert_eq!(
                    service
                        .clone()
                        .lookup_ptr(ptr_records.choose(&mut thread_rng()).unwrap().to_string())
                        .await
                        .first()
                        .unwrap(),
                    &record.to_string()
                );

                let mut ips = service.lookup_a(record.clone()).await;
                ips.sort();
                assert_eq!(ips, listen_ips.clone().to_ipv4_vec());
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_battery_single_domain_named() {
        let update_interval = Duration::new(2, 0);
        let service = Service::new(
            ServiceConfig::default()
                .update_interval(Some(update_interval))
                .ips(Some(vec!["172.16.240.2", "172.16.240.3", "172.16.240.4"])),
        )
        .await;

        let member_record = service.member_record();

        service.change_name("islay").await;

        let named_record = "islay.home.arpa.".to_string();

        for record in vec![member_record, named_record.clone()] {
            info!("Looking up {}", record);

            let mut listen_ips = service.listen_ips.clone();
            listen_ips.sort();

            for _ in 0..1000 {
                let mut ips = service.lookup_a(record.clone()).await;
                ips.sort();
                assert_eq!(ips, listen_ips.clone().to_ipv4_vec());
            }
        }

        let ptr_records: Vec<String> = service.listen_ips.clone().to_ptr_vec();

        for ptr_record in ptr_records {
            info!("Looking up {}", ptr_record);

            for _ in 0..1000 {
                let service = service.clone();
                assert_eq!(
                    service
                        .lookup_ptr(ptr_record.clone())
                        .await
                        .first()
                        .unwrap(),
                    &named_record.to_string()
                );
            }
        }
    }
}

mod all {
    use rand::prelude::SliceRandom;
    use tracing::info;
    use trust_dns_resolver::{IntoName, Name};

    use zeronsd::{addresses::Calculator, hosts::parse_hosts, utils::TEST_HOSTS_DIR};

    use crate::service::{resolver::Lookup, utils::HostsType, Service, ServiceConfig};

    use std::{
        net::{IpAddr, Ipv4Addr, Ipv6Addr},
        path::Path,
        str::FromStr,
        thread::sleep,
        time::Duration,
    };

    #[tokio::test(flavor = "multi_thread")]
    async fn test_battery_multi_domain_hosts_file() {
        let ips = vec!["172.16.240.2", "172.16.240.3", "172.16.240.4"];
        let service = Service::new(
            ServiceConfig::default()
                .hosts(HostsType::Fixture("basic"))
                .ips(Some(ips.clone())),
        )
        .await;

        let record = service.member_record();

        info!("Looking up random domains");

        let mut hosts_map = parse_hosts(
            Some(Path::new(&format!("{}/basic", TEST_HOSTS_DIR)).to_path_buf()),
            "home.arpa.".into_name().unwrap(),
        )
        .unwrap();

        for ip in ips {
            hosts_map.insert(
                IpAddr::from_str(&ip).unwrap(),
                vec![record.clone().into_name().unwrap()],
            );
        }

        let mut hosts = hosts_map.values().flatten().collect::<Vec<&Name>>();
        for _ in 0..1000 {
            hosts.shuffle(&mut rand::thread_rng());
            let host = *hosts.first().unwrap();
            let ips = service.lookup_a(host.to_string()).await;
            assert!(hosts_map
                .get(&IpAddr::from(*ips.first().unwrap()))
                .unwrap()
                .contains(host));
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_hosts_file_reloading() {
        let hosts_path = "/tmp/zeronsd-test-hosts";
        std::fs::write(hosts_path, "127.0.0.2 islay\n::2 islay\n").unwrap();
        let service = Service::new(
            ServiceConfig::default()
                .hosts(HostsType::Path(hosts_path))
                .update_interval(Some(Duration::new(2, 0))),
        )
        .await;

        assert_eq!(
            service
                .lookup_a("islay.home.arpa.".to_string())
                .await
                .first()
                .unwrap(),
            &Ipv4Addr::from_str("127.0.0.2").unwrap()
        );

        assert_eq!(
            service
                .lookup_aaaa("islay.home.arpa.".to_string())
                .await
                .first()
                .unwrap(),
            &Ipv6Addr::from_str("::2").unwrap()
        );

        std::fs::write(hosts_path, "127.0.0.3 islay\n::3 islay\n").unwrap();
        sleep(Duration::new(30, 0)); // wait for bg update

        assert_eq!(
            service
                .lookup_a("islay.home.arpa.".to_string())
                .await
                .first()
                .unwrap(),
            &Ipv4Addr::from_str("127.0.0.3").unwrap()
        );

        assert_eq!(
            service
                .lookup_aaaa("islay.home.arpa.".to_string())
                .await
                .first()
                .unwrap(),
            &Ipv6Addr::from_str("::3").unwrap()
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_listen_ip() -> Result<(), anyhow::Error> {
        use crate::service::{context::TestContext, network::TestNetwork};
        use zeronsd::utils::*;

        let tn = TestNetwork::new("basic-ipv4", &mut TestContext::default().await)
            .await
            .unwrap();

        let listen_ips = get_listen_ips(
            &authtoken_path(None),
            &tn.network.clone().id.unwrap(),
            ZEROTIER_LOCAL_URL.into(),
        )
        .await?;

        eprintln!("My listen IP is {}", listen_ips.first().unwrap());
        assert_ne!(*listen_ips.first().unwrap(), String::from(""));

        drop(tn);

        // see testdata/networks/basic-ipv4.json
        let mut ips = vec!["172.16.240.2", "172.16.240.3", "172.16.240.4"];
        let tn =
            TestNetwork::new_multi_ip("basic-ipv4", &mut TestContext::default().await, ips.clone())
                .await
                .unwrap();
        ips.sort();

        let mut listen_ips: Vec<String> = get_listen_ips(
            &authtoken_path(None),
            &tn.network.clone().id.unwrap(),
            ZEROTIER_LOCAL_URL.into(),
        )
        .await?
        .iter()
        .map(|x| parse_ip_from_cidr(x.clone()).to_string())
        .collect();
        listen_ips.sort();

        assert_eq!(listen_ips, ips);
        eprintln!("My listen IPs are {}", listen_ips.join(", "));

        let tn = TestNetwork::new("rfc4193-only", &mut TestContext::default().await)
            .await
            .unwrap();

        let mut listen_ips: Vec<String> = get_listen_ips(
            &authtoken_path(None),
            &tn.network.clone().id.unwrap(),
            ZEROTIER_LOCAL_URL.into(),
        )
        .await?
        .iter()
        .map(|x| parse_ip_from_cidr(x.clone()).to_string())
        .collect();
        listen_ips.sort();

        let mut ips = vec![tn.member().clone().rfc4193()?.ip().to_string()];
        ips.sort();

        assert_eq!(listen_ips, ips);
        eprintln!("My listen IPs are {}", listen_ips.join(", "));

        drop(tn);

        let tn = TestNetwork::new("6plane-only", &mut TestContext::default().await)
            .await
            .unwrap();

        let mut listen_ips: Vec<String> = get_listen_ips(
            &authtoken_path(None),
            &tn.network.clone().id.unwrap(),
            ZEROTIER_LOCAL_URL.into(),
        )
        .await?
        .iter()
        .map(|x| parse_ip_from_cidr(x.clone()).to_string())
        .collect();
        listen_ips.sort();

        let mut ips = vec![tn.member().clone().sixplane()?.ip().to_string()];
        ips.sort();

        assert_eq!(listen_ips, ips);
        eprintln!("My listen IPs are {}", listen_ips.join(", "));

        Ok(())
    }
}
