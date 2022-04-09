use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

type SocketVec = Vec<SocketAddr>;

pub trait ToIPv4Vec {
    fn to_ipv4_vec(self) -> Vec<Ipv4Addr>;
}

pub trait ToIPv6Vec {
    fn to_ipv6_vec(self) -> Vec<Ipv6Addr>;
}

pub trait ToPTRVec {
    fn to_ptr_vec(self) -> Vec<String>;
}

impl ToIPv4Vec for SocketVec {
    fn to_ipv4_vec(self) -> Vec<Ipv4Addr> {
        self.into_iter()
            .filter_map(|ip| match ip.ip() {
                IpAddr::V4(ip) => Some(ip),
                IpAddr::V6(_) => None,
            })
            .collect::<Vec<Ipv4Addr>>()
    }
}

impl ToIPv6Vec for SocketVec {
    fn to_ipv6_vec(self) -> Vec<Ipv6Addr> {
        self.into_iter()
            .filter_map(|ip| match ip.ip() {
                IpAddr::V4(_) => None,
                IpAddr::V6(ip) => Some(ip),
            })
            .collect::<Vec<Ipv6Addr>>()
    }
}

impl ToPTRVec for SocketVec {
    fn to_ptr_vec(self) -> Vec<String> {
        self.into_iter()
            .map(|ip| ip.ip().to_string())
            .collect::<Vec<String>>()
    }
}
