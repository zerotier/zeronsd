use std::{net::IpAddr, str::FromStr};

use openapi::models::Member;
use trust_dns_server::{
    client::rr::{rdata::SOA, Name, RData, Record},
    proto::error::ProtoError,
    store::in_memory::InMemoryAuthority,
};

use std::time::Duration;

use tokio::net::{TcpListener, UdpSocket};
use trust_dns_server::{authority::Catalog, server::ServerFuture};

pub async fn listen(
    catalog: Catalog,
    listen_addr: &str,
    tcp_timeout: Duration,
) -> Result<(), anyhow::Error> {
    let tcp = TcpListener::bind(listen_addr).await?;
    let udp = UdpSocket::bind(listen_addr).await?;
    let mut sf = ServerFuture::new(catalog);

    sf.register_socket(udp);
    sf.register_listener(tcp, tcp_timeout);

    sf.block_until_done().await?;
    Ok(())
}

pub fn new_authority(domain_name: &str) -> Result<InMemoryAuthority, ProtoError> {
    let domain_name = Name::from_str(domain_name)?;

    let mut authority = InMemoryAuthority::empty(
        domain_name.clone(),
        trust_dns_server::authority::ZoneType::Primary,
        false,
    );

    let mut soa = Record::with(
        domain_name.clone(),
        trust_dns_server::client::rr::RecordType::SOA,
        60,
    );

    soa.set_rdata(RData::SOA(SOA::new(
        domain_name.clone(),
        Name::from_str("postmaster.example.com")?,
        1,
        1200,
        10,
        -1,
        0,
    )));

    authority.upsert(soa, 1);
    Ok(authority)
}

pub fn configure_authority(
    authority: &mut InMemoryAuthority,
    domain_name: Name,
    initial_serial: u32,
    members: Vec<Member>,
) -> Result<u32, std::io::Error> {
    let mut serial = initial_serial;

    for member in members {
        let member_name = format!("zt-{}", member.node_id.unwrap());

        let fqdn = Name::from_str(&member_name)?.append_name(&domain_name.clone());

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
                            Name::from_str(&name)?.append_name(&domain_name.clone()),
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

    Ok(serial)
}
