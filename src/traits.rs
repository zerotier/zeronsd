use std::{net::IpAddr, str::FromStr};

use anyhow::anyhow;
use ipnetwork::IpNetwork;
use regex::Regex;
use trust_dns_resolver::{proto::error::ProtoError, IntoName, Name};
use trust_dns_server::client::rr::LowerName;
use zerotier_central_api::models::Member;

pub trait ToPointerSOA {
    fn to_ptr_soa_name(self) -> Result<LowerName, ProtoError>;
}

impl ToPointerSOA for IpNetwork {
    fn to_ptr_soa_name(self) -> Result<LowerName, ProtoError> {
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
    fn to_wildcard(self) -> Name;
}

impl ToWildcard for Name {
    fn to_wildcard(self) -> Name {
        let name = Self::from_str("*").unwrap();
        name.append_domain(&self).unwrap().into_wildcard()
    }
}

// translation_table should also be lazy_static and provides a small match set to find and correct
// problems with member namesl.
fn translation_table() -> Vec<(Regex, &'static str)> {
    vec![
        (Regex::new(r"\s+").unwrap(), "-"), // translate whitespace to `-`
        (Regex::new(r"[^.\s\w\d-]+").unwrap(), ""), // catch-all at the end
    ]
}

pub trait ToHostname {
    fn to_hostname(self) -> Result<Name, anyhow::Error>;
    fn to_fqdn(self, domain: Name) -> Result<Name, anyhow::Error>;
}

impl ToHostname for &str {
    fn to_hostname(self) -> Result<Name, anyhow::Error> {
        self.to_string().to_hostname()
    }

    fn to_fqdn(self, domain: Name) -> Result<Name, anyhow::Error> {
        Ok(self.to_hostname()?.append_domain(&domain).unwrap())
    }
}

impl ToHostname for IpAddr {
    fn to_hostname(self) -> Result<Name, anyhow::Error> {
        self.to_string().to_hostname()
    }

    fn to_fqdn(self, domain: Name) -> Result<Name, anyhow::Error> {
        self.to_string().to_fqdn(domain)
    }
}

impl ToHostname for Member {
    fn to_hostname(self) -> Result<Name, anyhow::Error> {
        ("zt-".to_string() + &self.node_id.unwrap()).to_hostname()
    }

    fn to_fqdn(self, domain: Name) -> Result<Name, anyhow::Error> {
        ("zt-".to_string() + &self.node_id.unwrap()).to_fqdn(domain)
    }
}

impl ToHostname for String {
    // to_hostname turns member names into trust-dns compatible dns names.
    fn to_hostname(self) -> Result<Name, anyhow::Error> {
        let mut s = self.trim().to_string();
        for (regex, replacement) in translation_table() {
            s = regex.replace_all(&s, replacement).to_string();
        }

        let s = s.trim();

        if s == "." || s.ends_with(".") {
            return Err(anyhow!("Record {} not entered into catalog: '.' and records that ends in '.' are disallowed", s));
        }

        if s.len() == 0 {
            return Err(anyhow!("translated hostname {} is an empty string", self));
        }

        Ok(s.trim().into_name()?)
    }

    fn to_fqdn(self, domain: Name) -> Result<Name, anyhow::Error> {
        Ok(self.to_hostname()?.append_domain(&domain).unwrap())
    }
}
