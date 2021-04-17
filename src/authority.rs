use std::{
    net::IpAddr,
    str::FromStr,
    sync::{Arc, Mutex, RwLock},
};

use anyhow::anyhow;

use openapi::{apis::configuration::Configuration, models::Member};
use trust_dns_server::{
    authority::{AuthorityObject, Catalog},
    client::rr::{Name, RData, Record},
    store::in_memory::InMemoryAuthority,
};

pub const DOMAIN_NAME: &str = "domain.";

pub struct ZTAuthority {
    authority: Box<Arc<RwLock<InMemoryAuthority>>>,
    domain_name: Name,
    serial: Arc<Mutex<u32>>,
    network: String,
    config: Configuration,
}

impl ZTAuthority {
    pub fn new(
        domain_name: Name,
        initial_serial: u32,
        network: String,
        config: Configuration,
    ) -> Result<Arc<Self>, anyhow::Error> {
        Ok(Arc::new(Self {
            serial: Arc::new(Mutex::new(initial_serial)),
            domain_name: domain_name.clone(),
            network,
            config,
            authority: Box::new(Arc::new(RwLock::new(InMemoryAuthority::empty(
                domain_name.clone(),
                trust_dns_server::authority::ZoneType::Primary,
                false,
            )))),
        }))
    }

    async fn get_members(self: Arc<Self>) -> Result<Vec<Member>, anyhow::Error> {
        let list =
            openapi::apis::network_member_api::get_network_member_list(&self.config, &self.network)
                .await?;
        Ok(list)
    }

    pub async fn find_members(self: Arc<Self>) {
        loop {
            eprintln!("finding members");
            match self.clone().get_members().await {
                Ok(members) => match self.clone().configure(members) {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("error configuring authority: {}", e)
                    }
                },
                Err(e) => {
                    eprintln!("error syncing members: {}", e)
                }
            }

            std::thread::sleep(std::time::Duration::new(30, 0));
        }
    }

    pub fn configure(self: Arc<Self>, members: Vec<Member>) -> Result<(), anyhow::Error> {
        let mut authority = match self.authority.write() {
            Ok(auth) => auth,
            Err(_) => return Err(anyhow!("could not acquire lock")),
        };

        for member in members {
            let member_name = format!("zt-{}", member.node_id.unwrap());
            let fqdn = Name::from_str(&member_name)?.append_name(&self.domain_name.clone());
            let mut serial = *(self.serial.lock().unwrap());

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

                        authority.upsert(address, serial);
                        if let Some(name) = member.name.clone() {
                            let mut address = Record::with(
                                Name::from_str(&name)?.append_name(&self.domain_name.clone()),
                                trust_dns_server::client::rr::RecordType::A,
                                60,
                            );
                            address.set_rdata(RData::A(ip));
                            serial += 1;
                            authority.upsert(address, serial);
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
                        authority.upsert(address, serial);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn catalog(&self) -> Catalog {
        let mut catalog = Catalog::default();
        catalog.upsert(self.domain_name.clone().into(), self.authority.box_clone());
        catalog
    }
}
