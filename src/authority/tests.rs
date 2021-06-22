use log::info;
use rand::{prelude::SliceRandom, thread_rng};
use trust_dns_resolver::{IntoName, Name};

use crate::{
    authority::service::{HostsType, Service, ServiceConfig},
    hosts::parse_hosts,
    integration_tests::init_test_logger,
    tests::HOSTS_DIR,
};

use std::{
    net::{IpAddr, Ipv4Addr},
    str::FromStr,
    thread::sleep,
    time::Duration,
};

#[test]
#[ignore]
fn test_wildcard_ipv4_central() {
    init_test_logger();
    let service = Service::new(
        ServiceConfig::default()
            .update_interval(Some(Duration::new(1, 0)))
            .wildcard_everything(true),
    );

    let member_record = service.member_record();
    let named_record = Name::from_str("islay.domain.").unwrap();

    service.change_name("islay");

    assert_eq!(
        service.lookup_a(named_record.to_string()).first().unwrap(),
        &service.any_listen_ip(),
    );

    assert_eq!(
        service.lookup_a(member_record.to_string()).first().unwrap(),
        &service.any_listen_ip(),
    );

    for host in vec!["one", "ten", "zt-foo", "another-record"] {
        for rec in vec![named_record.to_string(), member_record.clone()] {
            let lookup = Name::from_str(&host)
                .unwrap()
                .append_domain(&Name::from_str(&rec).unwrap())
                .to_string();
            assert_eq!(
                service.lookup_a(lookup).first().unwrap(),
                &service.any_listen_ip()
            );
        }
    }
}

#[test]
#[ignore]
fn test_hosts_file_reloading() {
    init_test_logger();
    let hosts_path = "/tmp/zeronsd-test-hosts";
    std::fs::write(hosts_path, "127.0.0.2 islay\n").unwrap();
    let service = Service::new(
        ServiceConfig::default()
            .hosts(HostsType::Path(hosts_path))
            .update_interval(Some(Duration::new(1, 0))),
    );

    assert_eq!(
        service
            .lookup_a("islay.domain.".to_string())
            .first()
            .unwrap(),
        &Ipv4Addr::from_str("127.0.0.2").unwrap()
    );

    std::fs::write(hosts_path, "127.0.0.3 islay\n").unwrap();
    sleep(Duration::new(3, 0)); // wait for bg update

    assert_eq!(
        service
            .lookup_a("islay.domain.".to_string())
            .first()
            .unwrap(),
        &Ipv4Addr::from_str("127.0.0.3").unwrap()
    );
}

#[test]
#[ignore]
fn test_battery_single_domain() {
    init_test_logger();
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
    init_test_logger();
    let ips = vec!["172.16.240.2", "172.16.240.3", "172.16.240.4"];
    let service = Service::new(
        ServiceConfig::default()
            .hosts(HostsType::Fixture("basic"))
            .ips(Some(ips.clone())),
    );

    let record = service.member_record();

    info!("Looking up random domains");

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
#[ignore]
fn test_battery_single_domain_named() {
    init_test_logger();
    let update_interval = Duration::new(1, 0);
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
