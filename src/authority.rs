use std::{net::IpAddr, str::FromStr};

use openapi::models::Member;
use trust_dns_server::{
    authority::Catalog,
    client::rr::{Name, RData, Record},
    store::in_memory::InMemoryAuthority,
};

pub const DOMAIN_NAME: &str = "domain.";

pub struct ZeroAuthority {
    authority: InMemoryAuthority,
    domain_name: Name,
}

impl Default for ZeroAuthority {
    fn default() -> Self {
        Self::new(Name::from_str(DOMAIN_NAME).unwrap()).unwrap()
    }
}

impl ZeroAuthority {
    pub fn new(domain_name: Name) -> Result<Self, anyhow::Error> {
        Ok(Self {
            domain_name: domain_name.clone(),
            authority: InMemoryAuthority::empty(
                domain_name.clone(),
                trust_dns_server::authority::ZoneType::Primary,
                false,
            ),
        })
    }

    pub fn configure(
        &mut self,
        initial_serial: u32,
        members: Vec<Member>,
    ) -> Result<u32, anyhow::Error> {
        let mut serial = initial_serial;

        for member in members {
            let member_name = format!("zt-{}", member.node_id.unwrap());

            let fqdn = Name::from_str(&member_name)?.append_name(&self.domain_name.clone());

            for ip in member.config.unwrap().ip_assignments.unwrap() {
                match IpAddr::from_str(&ip).unwrap() {
                    IpAddr::V4(ip) => {
                        let mut address = Record::with(
                            fqdn.clone(),
                            trust_dns_server::client::rr::RecordType::A,
                            60,
                        );
                        address.set_rdata(RData::A(ip));
                        serial += 1;
                        self.authority.upsert(address, serial);
                        if let Some(name) = member.name.clone() {
                            let mut address = Record::with(
                                Name::from_str(&name)?.append_name(&self.domain_name.clone()),
                                trust_dns_server::client::rr::RecordType::A,
                                60,
                            );
                            address.set_rdata(RData::A(ip));
                            serial += 1;
                            self.authority.upsert(address, serial);
                        }
                    }
                    IpAddr::V6(ip) => {
                        let mut address = Record::with(
                            fqdn.clone(),
                            trust_dns_server::client::rr::RecordType::AAAA,
                            60,
                        );
                        address.set_rdata(RData::AAAA(ip));
                        serial += 1;
                        self.authority.upsert(address, serial);
                    }
                }
            }
        }

        Ok(serial)
    }

    pub fn catalog(self) -> Catalog {
        let mut catalog = Catalog::default();
        catalog.upsert(
            self.domain_name.clone().into(),
            Box::new(std::sync::Arc::new(std::sync::RwLock::new(self.authority))),
        );
        catalog
    }
}
