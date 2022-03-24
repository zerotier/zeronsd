use ipnetwork::IpNetwork;
use std::str::FromStr;
use trust_dns_resolver::{proto::error::ProtoError, IntoName, Name};
use trust_dns_server::client::rr::LowerName;

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
