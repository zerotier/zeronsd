use std::{
    net::IpAddr,
    path::{Path, PathBuf},
    str::FromStr,
};

use crate::traits::ToHostname;
use crate::utils::domain_or_default;

#[test]
fn test_parse_member_name() {
    use crate::utils::parse_member_name;

    let actual_domains: &mut Vec<Option<&str>> =
        &mut vec!["tld", "domain", "zerotier", "test.subdomain"]
            .iter()
            .map(|s| Some(*s))
            .collect::<Vec<Option<&str>>>();

    actual_domains.push(None); // make sure the None case also gets checked

    for domain in actual_domains {
        let domain_name = domain_or_default(*domain).unwrap().clone();

        assert_eq!(parse_member_name(None, domain_name.clone()), None);

        for name in ["islay", "ALL-CAPS", "Capitalized", "with.dots"] {
            assert_eq!(
                parse_member_name(Some(name.to_string()), domain_name.clone()),
                Some(name.to_fqdn(domain_name.clone()).unwrap()),
                "{}",
                name,
            );
        }

        for bad_name in [".", "!", "arghle."] {
            assert_eq!(
                parse_member_name(Some(bad_name.to_string()), domain_name.clone()),
                None,
                "{}",
                bad_name,
            );
        }

        for (orig, translated) in [("Erik's laptop", "eriks-laptop"), ("!foo", "foo")] {
            assert_eq!(
                parse_member_name(Some(orig.to_string()), domain_name.clone()),
                Some(translated.to_fqdn(domain_name.clone()).unwrap()),
                "{}",
                orig,
            );
        }
    }
}

#[test]
fn test_parse_ip_from_cidr() {
    use crate::utils::parse_ip_from_cidr;

    let results = vec![
        ("192.168.12.1/16", "192.168.12.1"),
        ("10.0.0.0/8", "10.0.0.0"),
        ("fe80::abcd/128", "fe80::abcd"),
    ];

    for (cidr, ip) in results {
        assert_eq!(
            parse_ip_from_cidr(String::from(cidr)),
            IpAddr::from_str(ip).unwrap(),
            "{}",
            cidr
        );
    }
}

#[test]
fn test_domain_or_default() {
    use crate::utils::{domain_or_default, DOMAIN_NAME};
    use std::str::FromStr;
    use trust_dns_server::client::rr::Name;

    assert_eq!(
        domain_or_default(None).unwrap(),
        Name::from_str(DOMAIN_NAME).unwrap()
    );

    assert_eq!(
        domain_or_default(Some("zerotier")).unwrap(),
        Name::from_str("zerotier").unwrap()
    );

    assert_eq!(
        domain_or_default(Some("zerotier.tld")).unwrap(),
        Name::from_str("zerotier.tld").unwrap()
    );

    for bad in ["bad.", "~", "!", ".", ""] {
        assert!(domain_or_default(Some(bad)).is_err(), "{}", bad);
    }
}

#[test]
fn test_central_token() {
    use crate::utils::central_token;

    assert!(central_token(None).is_err());
    std::env::set_var("ZEROTIER_CENTRAL_TOKEN", "abcdef");
    assert_eq!(central_token(None).unwrap(), "abcdef");

    let hosts = std::fs::read_to_string("/etc/hosts").unwrap();
    let token = central_token(Some(Path::new("/etc/hosts")));
    assert!(token.is_ok());
    assert_eq!(token.unwrap(), hosts.trim());
}

#[test]
#[should_panic]
fn test_central_token_panic() {
    use crate::utils::central_token;
    central_token(Some(Path::new("/nonexistent"))).unwrap();
}

#[test]
#[cfg(target_os = "linux")]
fn test_supervise_systemd_green() {
    use std::path::PathBuf;

    let table = vec![
        (
            "basic",
            crate::supervise::Properties {
                binpath: String::from("zeronsd"),
                network: String::from("1234567891011121"),
                token: PathBuf::from("/proc/cpuinfo"),
                ..Default::default()
            },
        ),
        (
            "with-filled-in-properties",
            crate::supervise::Properties {
                binpath: String::from("zeronsd"),
                network: String::from("1234567891011121"),
                token: PathBuf::from("/proc/cpuinfo"),
                domain: Some(String::from("zerotier")),
                authtoken: Some(PathBuf::from("/var/lib/zerotier-one/authtoken.secret")),
                hosts_file: Some(PathBuf::from("/etc/hosts")),
                wildcard_names: true,
                distro: None,
                ..Default::default()
            },
        ),
    ];

    let write = match std::env::var("WRITE_FIXTURES") {
        Ok(var) => var != "",
        Err(_) => false,
    };

    if write {
        eprintln!("Write mode: not testing, but updating unit files")
    }

    for (name, mut props) in table {
        let path = std::path::PathBuf::from(format!("testdata/supervise/systemd/{}.unit", name));

        if !write {
            let path = path.canonicalize();

            let expected = std::fs::read_to_string(path.unwrap());
            let testing = props.supervise_template();

            assert_eq!(testing.unwrap(), expected.unwrap(), "{}", name);
        } else {
            assert!(props.validate().is_ok(), "{}", name);

            let template = props.supervise_template();
            assert!(template.is_ok(), "{}", name);
            assert!(
                std::fs::write(path, props.supervise_template().unwrap()).is_ok(),
                "{}",
                name
            );
        }
    }
}

#[test]
#[cfg(target_os = "linux")]
fn test_supervise_systemd_red() {
    use std::path::PathBuf;

    let table = vec![
        (
            "bad network",
            crate::supervise::Properties {
                binpath: String::from("zeronsd"),
                network: String::from("123456789101112"),
                token: PathBuf::from("/proc/cpuinfo"),
                ..Default::default()
            },
        ),
        (
            "bad token (no file)",
            crate::supervise::Properties {
                binpath: String::from("zeronsd"),
                network: String::from("1234567891011121"),
                token: PathBuf::from("~"),
                ..Default::default()
            },
        ),
        (
            "bad token (dir)",
            crate::supervise::Properties {
                binpath: String::from("zeronsd"),
                network: String::from("1234567891011121"),
                token: PathBuf::from("."),
                ..Default::default()
            },
        ),
        (
            "bad hosts (no file)",
            crate::supervise::Properties {
                binpath: String::from("zeronsd"),
                network: String::from("1234567891011121"),
                token: PathBuf::from("/proc/cpuinfo"),
                hosts_file: Some(PathBuf::from("~")),
                ..Default::default()
            },
        ),
        (
            "bad hosts (dir)",
            crate::supervise::Properties {
                binpath: String::from("zeronsd"),
                network: String::from("1234567891011121"),
                token: PathBuf::from("/proc/cpuinfo"),
                hosts_file: Some(PathBuf::from(".")),
                ..Default::default()
            },
        ),
        (
            "bad authtoken (no file)",
            crate::supervise::Properties {
                binpath: String::from("zeronsd"),
                network: String::from("1234567891011121"),
                token: PathBuf::from("/proc/cpuinfo"),
                authtoken: Some(PathBuf::from("~")),
                ..Default::default()
            },
        ),
        (
            "bad authtoken (dir)",
            crate::supervise::Properties {
                binpath: String::from("zeronsd"),
                network: String::from("1234567891011121"),
                token: PathBuf::from("/proc/cpuinfo"),
                authtoken: Some(PathBuf::from(".")),
                ..Default::default()
            },
        ),
        (
            "bad domain (empty string)",
            crate::supervise::Properties {
                binpath: String::from("zeronsd"),
                network: String::from("1234567891011121"),
                token: PathBuf::from("/proc/cpuinfo"),
                domain: Some(String::from("")),
                ..Default::default()
            },
        ),
        (
            "bad domain (invalid)",
            crate::supervise::Properties {
                binpath: String::from("zeronsd"),
                network: String::from("1234567891011121"),
                token: PathBuf::from("/proc/cpuinfo"),
                domain: Some(String::from("-")),
                ..Default::default()
            },
        ),
    ];

    for (name, mut props) in table {
        assert!(props.validate().is_err(), "{}", name);
    }
}

#[test]
fn test_parse_hosts() {
    use crate::hosts::parse_hosts;
    use std::net::IpAddr;
    use std::str::FromStr;
    use trust_dns_resolver::Name;

    let domain = &Name::from_str("zombocom").unwrap();

    for path in std::fs::read_dir(crate::utils::TEST_HOSTS_DIR)
        .unwrap()
        .into_iter()
        .map(|p| p.unwrap())
    {
        if path.metadata().unwrap().is_file() {
            eprintln!("Testing: {}", path.path().display());
            let res = parse_hosts(Some(path.path()), domain.clone());
            assert!(res.is_ok(), "{}", path.path().display());

            let mut table = res.unwrap();

            assert_eq!(
                table
                    .remove(&IpAddr::from_str("127.0.0.1").unwrap())
                    .unwrap()
                    .first()
                    .unwrap(),
                &Name::from_str("localhost")
                    .unwrap()
                    .append_domain(domain)
                    .unwrap(),
                "{}",
                path.path().display(),
            );

            assert_eq!(
                table
                    .remove(&IpAddr::from_str("::1").unwrap())
                    .unwrap()
                    .first()
                    .unwrap(),
                &Name::from_str("localhost")
                    .unwrap()
                    .append_domain(domain)
                    .unwrap(),
                "{}",
                path.path().display(),
            );

            let mut accounted = vec!["islay.localdomain", "islay"]
                .into_iter()
                .map(|s| Name::from_str(s).unwrap().append_domain(domain).unwrap());

            for name in table
                .remove(&IpAddr::from_str("127.0.1.1").unwrap())
                .unwrap()
            {
                assert!(accounted.any(|s| s.eq(&name)));
            }
        }
    }
}

#[test]
fn test_parse_hosts_duplicate() {
    use crate::hosts::parse_hosts;
    use trust_dns_resolver::Name;

    let domain = Name::from_str("zombocom").unwrap();

    let res = parse_hosts(
        Some(PathBuf::from("testdata/hosts-files/duplicates")),
        domain.clone(),
    );

    assert!(res.is_ok());

    let table = res.unwrap();
    let result = table.get(&IpAddr::from_str("10.147.20.216").unwrap());
    assert!(result.is_some());
    let result = result.unwrap();

    assert!(result.contains(
        &Name::from_str("hostname1")
            .unwrap()
            .append_domain(&domain)
            .unwrap()
    ));
    assert!(result.contains(
        &Name::from_str("hostname2.corp")
            .unwrap()
            .append_domain(&domain)
            .unwrap()
    ));
}
