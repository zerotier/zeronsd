/// various IP calculation systems and some encode/decode functions
use std::net::{IpAddr, Ipv6Addr};

use hex::FromHexError;
use ipnetwork::IpNetwork;
use zerotier_api::central_api::types::{Member, Network};

fn digest_hex(code: String) -> Result<u64, FromHexError> {
    Ok(hex::decode(code)?
        .into_iter()
        .fold(0, |acc, x| acc << 8 | x as u64))
}

fn get_parts(member: Member) -> Result<(u64, u64), anyhow::Error> {
    Ok((
        digest_hex(member.network_id.clone().unwrap_or(String::new()))?,
        digest_hex(member.node_id.unwrap_or(String::new()))?,
    ))
}

pub trait Calculator {
    fn sixplane(self) -> Result<IpNetwork, anyhow::Error>;
    fn rfc4193(self) -> Result<IpNetwork, anyhow::Error>;
}

impl Calculator for Network {
    fn sixplane(self) -> Result<IpNetwork, anyhow::Error> {
        let mut net_parts = digest_hex(self.id.unwrap_or(String::new()))?;

        net_parts ^= net_parts >> 32;

        Ok(IpNetwork::new(
            IpAddr::V6(Ipv6Addr::new(
                0xfc00 | (net_parts >> 24 & 0xff) as u16,
                (net_parts >> 8) as u16,
                ((net_parts & 0xff) as u16) << 8,
                0,
                0,
                0,
                0,
                1,
            )),
            40,
        )?)
    }

    fn rfc4193(self) -> Result<IpNetwork, anyhow::Error> {
        let net_parts = digest_hex(self.id.unwrap_or(String::new()))?;
        Ok(IpNetwork::new(
            IpAddr::V6(Ipv6Addr::new(
                0xfd00 | (net_parts >> 56 & 0xff) as u16,
                (net_parts >> 40 & 0xffff) as u16,
                (net_parts >> 24 & 0xffff) as u16,
                (net_parts >> 8 & 0xffff) as u16,
                (((net_parts & 0xff) as u16) << 8) | 0x99,
                0x9300,
                0,
                0,
            )),
            88,
        )?)
    }
}

impl Calculator for Member {
    fn sixplane(self) -> Result<IpNetwork, anyhow::Error> {
        let (mut net_parts, node_parts) = get_parts(self)?;

        net_parts ^= net_parts >> 32;

        Ok(IpNetwork::new(
            IpAddr::V6(Ipv6Addr::new(
                0xfc00 | (net_parts >> 24 & 0xff) as u16,
                (net_parts >> 8) as u16,
                (((net_parts & 0xff) as u16) << 8) | ((node_parts >> 32 & 0xff) as u16),
                (node_parts >> 16 & 0xffff) as u16,
                (node_parts & 0xffff) as u16,
                0,
                0,
                1,
            )),
            80,
        )?)
    }

    fn rfc4193(self) -> Result<IpNetwork, anyhow::Error> {
        let (net_parts, node_parts) = get_parts(self)?;

        Ok(IpNetwork::new(
            IpAddr::V6(Ipv6Addr::new(
                0xfd00 | (net_parts >> 56 & 0xff) as u16,
                (net_parts >> 40 & 0xffff) as u16,
                (net_parts >> 24 & 0xffff) as u16,
                (net_parts >> 8 & 0xffff) as u16,
                (((net_parts & 0xff) as u16) << 8) | 0x99,
                0x9300 | (node_parts >> 32 & 0xff) as u16,
                (node_parts >> 16 & 0xffff) as u16,
                (node_parts & 0xffff) as u16,
            )),
            128,
        )?)
    }
}
