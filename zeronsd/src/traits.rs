use std::str::FromStr;

use anyhow::anyhow;
use ipnetwork::IpNetwork;
use lazy_static::lazy_static;
use regex::Regex;
use trust_dns_resolver::{proto::error::ProtoError, IntoName, Name};
use trust_dns_server::client::rr::LowerName;
use zerotier_central_api::types::Member;

pub trait ToPointerSOA {
    fn to_ptr_soa_name(&self) -> Result<LowerName, ProtoError>;
}

impl ToPointerSOA for IpNetwork {
    fn to_ptr_soa_name(&self) -> Result<LowerName, ProtoError> {
        // how many bits in each ptr octet
        let octet_factor = match self {
            IpNetwork::V4(_) => 8,
            IpNetwork::V6(_) => 4,
        };

        Ok(self
            .network()
            .into_name()?
            // round off the subnet, account for in-addr.arpa.
            .trim_to((self.prefix() as usize / octet_factor) + 2)
            .into())
    }
}

pub trait ToWildcard {
    fn to_wildcard(&self) -> Name;
}

impl ToWildcard for Name {
    fn to_wildcard(&self) -> Name {
        let name = Self::from_str("*").unwrap();
        name.append_domain(self).unwrap().into_wildcard()
    }
}

lazy_static! {
    static ref TRANSLATION_TABLE: Box<[(Regex, &'static str)]> = Box::new([
        (Regex::new(r"\s+").unwrap(), "-"), // translate whitespace to `-`
        (Regex::new(r"[^.\s\w\d-]+").unwrap(), ""), // catch-all at the end
    ]);
}

pub trait ToHostname {
    fn to_hostname(&self) -> Result<Name, anyhow::Error>;
    fn to_fqdn(&self, domain: Name) -> Result<Name, anyhow::Error>;
}

impl ToHostname for &str {
    fn to_hostname(&self) -> Result<Name, anyhow::Error> {
        self.to_string().to_hostname()
    }

    fn to_fqdn(&self, domain: Name) -> Result<Name, anyhow::Error> {
        Ok(self.to_hostname()?.append_domain(&domain).unwrap())
    }
}

impl ToHostname for Member {
    fn to_hostname(&self) -> Result<Name, anyhow::Error> {
        ("zt-".to_string() + &self.node_id.clone().unwrap()).to_hostname()
    }

    fn to_fqdn(&self, domain: Name) -> Result<Name, anyhow::Error> {
        ("zt-".to_string() + &self.node_id.clone().unwrap()).to_fqdn(domain)
    }
}

impl ToHostname for String {
    // to_hostname turns member names into trust-dns compatible dns names.
    fn to_hostname(&self) -> Result<Name, anyhow::Error> {
        let mut s = self.trim().to_string();
        for (regex, replacement) in TRANSLATION_TABLE.iter() {
            s = regex.replace_all(&s, *replacement).to_string();
        }

        let s = s.trim();

        if s == "." || s.ends_with('.') {
            return Err(anyhow!("Record {} not entered into catalog: '.' and records that ends in '.' are disallowed", s));
        }

        if s.is_empty() {
            return Err(anyhow!("translated hostname {} is an empty string", self));
        }

        Ok(s.trim().into_name()?)
    }

    fn to_fqdn(&self, domain: Name) -> Result<Name, anyhow::Error> {
        Ok(self.to_hostname()?.append_domain(&domain).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::{ToHostname, ToPointerSOA, ToWildcard};
    use ipnetwork::IpNetwork;
    use trust_dns_resolver::Name;
    use trust_dns_server::client::rr::LowerName;
    use zerotier_central_api::types::Member;

    #[test]
    fn test_to_ptr_soa_name() {
        for item in vec![
            (
                IpNetwork::from_str("1.2.3.4/24").unwrap(),
                LowerName::from_str("3.2.1.in-addr.arpa").unwrap(),
            ),
            (
                IpNetwork::from_str("1.2.3.4/16").unwrap(),
                LowerName::from_str("2.1.in-addr.arpa").unwrap(),
            ),
            (
                IpNetwork::from_str("1.2.3.4/8").unwrap(),
                LowerName::from_str("1.in-addr.arpa").unwrap(),
            ),
            (
                IpNetwork::from_str("1.2.3.4/12").unwrap(),
                LowerName::from_str("1.in-addr.arpa").unwrap(),
            ),
            (
                IpNetwork::from_str("1.2.3.4/22").unwrap(),
                LowerName::from_str("2.1.in-addr.arpa").unwrap(),
            ),
            (
                IpNetwork::from_str("1.2.3.4/26").unwrap(),
                LowerName::from_str("3.2.1.in-addr.arpa").unwrap(),
            ),
            (
                IpNetwork::from_str("1.2.3.4/32").unwrap(),
                LowerName::from_str("4.3.2.1.in-addr.arpa").unwrap(),
            ),
        ] {
            assert_eq!(item.0.to_ptr_soa_name().unwrap(), item.1);
        }
    }

    #[test]
    fn test_to_wildcard() {
        let hostname = "test.home.arpa".to_hostname().unwrap();
        let wildcard = hostname.to_wildcard();
        assert_eq!(wildcard.to_string(), "*.test.home.arpa.");
    }

    #[test]
    fn test_to_hostname_member() {
        let mut member = Member {
            supports_rules_engine: None,
            protocol_version: None,
            physical_address: None,
            node_id: None,
            network_id: None,
            name: None,
            last_online: None,
            last_seen: None,
            id: None,
            hidden: None,
            description: None,
            controller_id: None,
            config: None,
            client_version: None,
            clock: None,
        };
        member.node_id = Some("foo".to_string());
        let hostname = member.to_hostname().unwrap();
        assert_eq!(hostname, Name::from_str("zt-foo").unwrap());
        let fqdn = member
            .to_fqdn(Name::from_str("home.arpa").unwrap())
            .unwrap();
        assert_eq!(fqdn, Name::from_str("zt-foo.home.arpa").unwrap());

        member.node_id = Some("Joe Sixpack's iMac".to_string());
        let hostname = member.to_hostname().unwrap();
        assert_eq!(hostname, Name::from_str("zt-joe-sixpacks-imac").unwrap());
        let fqdn = member
            .to_fqdn(Name::from_str("home.arpa").unwrap())
            .unwrap();
        assert_eq!(
            fqdn,
            Name::from_str("zt-joe-sixpacks-imac.home.arpa").unwrap()
        );

        member.node_id = Some("abc.".to_string());
        assert!(member.to_hostname().is_err());
        assert!(member
            .to_fqdn(Name::from_str("home.arpa").unwrap())
            .is_err());
    }

    #[test]
    fn test_to_hostname_string_str() {
        let hostname = "foo".to_hostname().unwrap();
        assert_eq!(hostname, Name::from_str("foo").unwrap());
        let fqdn = "foo".to_fqdn(Name::from_str("home.arpa").unwrap()).unwrap();
        assert_eq!(fqdn, Name::from_str("foo.home.arpa").unwrap());

        let hostname = "foo".to_string().to_hostname().unwrap();
        assert_eq!(hostname, Name::from_str("foo").unwrap());
        let fqdn = "foo"
            .to_string()
            .to_fqdn(Name::from_str("home.arpa").unwrap())
            .unwrap();
        assert_eq!(fqdn, Name::from_str("foo.home.arpa").unwrap());

        let hostname = "Joe Sixpack's iMac".to_hostname().unwrap();
        assert_eq!(hostname, Name::from_str("joe-sixpacks-imac").unwrap());
        let fqdn = "Joe Sixpack's iMac"
            .to_fqdn(Name::from_str("home.arpa").unwrap())
            .unwrap();
        assert_eq!(fqdn, Name::from_str("joe-sixpacks-imac.home.arpa").unwrap());

        let hostname = "Joe Sixpack's iMac".to_string().to_hostname().unwrap();
        assert_eq!(hostname, Name::from_str("joe-sixpacks-imac").unwrap());
        let fqdn = "Joe Sixpack's iMac"
            .to_string()
            .to_fqdn(Name::from_str("home.arpa").unwrap())
            .unwrap();
        assert_eq!(fqdn, Name::from_str("joe-sixpacks-imac.home.arpa").unwrap());

        assert!("abc.".to_hostname().is_err());
        assert!("abc."
            .to_fqdn(Name::from_str("home.arpa").unwrap())
            .is_err());

        assert!("abc.".to_string().to_hostname().is_err());
        assert!("abc."
            .to_string()
            .to_fqdn(Name::from_str("home.arpa").unwrap())
            .is_err());
    }
}
