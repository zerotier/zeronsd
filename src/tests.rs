struct TestParams {
    central_token: String,
    network: String,
}

/// Set TOKEN and NETWORK to activate integration tests, otherwise they will pass silently.
/// Requires a pre-configured zerotier network.
/// You must also be root, and the `-- --ignored` flag must be passed to cargo test.
fn integration_test_params() -> TestParams {
    if let Ok(central_token) = std::env::var("TOKEN") {
        if central_token.trim().len() > 0 {
            if let Ok(network) = std::env::var("NETWORK") {
                if network.trim().len() > 0 {
                    eprintln!("Integration tests activated!");
                    return TestParams {
                        central_token,
                        network,
                    };
                }
            }
        }
    }

    panic!("Please provide TOKEN and NETWORK to run these tests");
}

#[test]
fn test_parse_ip_from_cidr() {
    use crate::parse_ip_from_cidr;

    let results = vec![
        ("192.168.12.1/16", "192.168.12.1"),
        ("10.0.0.0/8", "10.0.0.0"),
        ("fe80::abcd/128", "fe80::abcd"),
    ];

    for (cidr, ip) in results {
        assert_eq!(
            parse_ip_from_cidr(String::from(cidr)),
            String::from(ip),
            "{}",
            cidr
        );
    }
}

#[test]
fn test_domain_or_default() {
    use crate::{authority::DOMAIN_NAME, domain_or_default};
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

    for bad in vec!["bad.", "~", "!", ".", ""] {
        assert!(domain_or_default(Some(bad)).is_err(), "{}", bad);
    }
}

#[test]
fn test_central_token() {
    use crate::central_token;

    assert!(central_token(None).is_none());
    std::env::set_var("ZEROTIER_CENTRAL_TOKEN", "abcdef");
    assert!(central_token(None).is_some());
    assert_eq!(central_token(None).unwrap(), "abcdef");

    let hosts = std::fs::read_to_string("/etc/hosts").unwrap();
    let token = central_token(Some("/etc/hosts"));
    assert!(token.is_some());
    assert_eq!(token.unwrap(), hosts.trim());
}

#[test]
#[should_panic]
fn test_central_token_panic() {
    use crate::central_token;
    central_token(Some("/nonexistent"));
}

#[test]
#[ignore]
fn test_get_listen_ip() -> Result<(), anyhow::Error> {
    use crate::{authtoken_path, get_listen_ip, init_runtime};

    let test_params = integration_test_params();
    let runtime = init_runtime();
    let authtoken = authtoken_path(None);

    let listen_ip = runtime.block_on(get_listen_ip(&authtoken, &test_params.network))?;
    assert_ne!(listen_ip, String::from(""));

    Ok(())
}

#[test]
#[cfg(target_os = "linux")]
fn test_supervise_systemd_green() {
    let table = vec![
        (
            "basic",
            crate::supervise::Properties {
                binpath: String::from("zeronsd"),
                network: String::from("1234567891011121"),
                token: String::from("/proc/cpuinfo"),
                ..Default::default()
            },
        ),
        (
            "with-filled-in-properties",
            crate::supervise::Properties {
                binpath: String::from("zeronsd"),
                network: String::from("1234567891011121"),
                token: String::from("/proc/cpuinfo"),
                domain: Some(String::from("zerotier")),
                authtoken: Some(String::from("/var/lib/zerotier-one/authtoken.secret")),
                hosts_file: Some(String::from("/etc/hosts")),
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

            assert!(path.is_ok(), "{}", name);
            let expected = std::fs::read_to_string(path.unwrap());
            assert!(expected.is_ok(), "{}", name);
            let testing = props.systemd_template();
            assert!(testing.is_ok(), "{}", name);

            assert_eq!(testing.unwrap(), expected.unwrap(), "{}", name);
        } else {
            assert!(props.validate().is_ok(), "{}", name);

            let template = props.systemd_template();
            assert!(template.is_ok(), "{}", name);
            assert!(
                std::fs::write(path, props.systemd_template().unwrap()).is_ok(),
                "{}",
                name
            );
        }
    }
}

#[test]
#[cfg(target_os = "linux")]
fn test_supervise_systemd_red() {
    let table = vec![
        (
            "bad network",
            crate::supervise::Properties {
                binpath: String::from("zeronsd"),
                network: String::from("123456789101112"),
                token: String::from("/proc/cpuinfo"),
                ..Default::default()
            },
        ),
        (
            "bad token (no file)",
            crate::supervise::Properties {
                binpath: String::from("zeronsd"),
                network: String::from("1234567891011121"),
                token: String::from("~"),
                ..Default::default()
            },
        ),
        (
            "bad token (dir)",
            crate::supervise::Properties {
                binpath: String::from("zeronsd"),
                network: String::from("1234567891011121"),
                token: String::from("."),
                ..Default::default()
            },
        ),
        (
            "bad hosts (no file)",
            crate::supervise::Properties {
                binpath: String::from("zeronsd"),
                network: String::from("1234567891011121"),
                token: String::from("/proc/cpuinfo"),
                hosts_file: Some(String::from("~")),
                ..Default::default()
            },
        ),
        (
            "bad hosts (dir)",
            crate::supervise::Properties {
                binpath: String::from("zeronsd"),
                network: String::from("1234567891011121"),
                token: String::from("/proc/cpuinfo"),
                hosts_file: Some(String::from(".")),
                ..Default::default()
            },
        ),
        (
            "bad authtoken (no file)",
            crate::supervise::Properties {
                binpath: String::from("zeronsd"),
                network: String::from("1234567891011121"),
                token: String::from("/proc/cpuinfo"),
                authtoken: Some(String::from("~")),
                ..Default::default()
            },
        ),
        (
            "bad authtoken (dir)",
            crate::supervise::Properties {
                binpath: String::from("zeronsd"),
                network: String::from("1234567891011121"),
                token: String::from("/proc/cpuinfo"),
                authtoken: Some(String::from(".")),
                ..Default::default()
            },
        ),
        (
            "bad domain (empty string)",
            crate::supervise::Properties {
                binpath: String::from("zeronsd"),
                network: String::from("1234567891011121"),
                token: String::from("/proc/cpuinfo"),
                domain: Some(String::from("")),
                ..Default::default()
            },
        ),
        (
            "bad domain (invalid)",
            crate::supervise::Properties {
                binpath: String::from("zeronsd"),
                network: String::from("1234567891011121"),
                token: String::from("/proc/cpuinfo"),
                domain: Some(String::from("-")),
                ..Default::default()
            },
        ),
    ];

    for (name, mut props) in table {
        assert!(props.validate().is_err(), "{}", name);
    }
}
