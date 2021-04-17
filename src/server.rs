use std::time::Duration;
use tokio::net::{TcpListener, UdpSocket};

use trust_dns_server::server::ServerFuture;

use crate::authority::ZTAuthority;

pub struct Server {
    authority: ZTAuthority,
}

impl Server {
    pub fn new(authority: ZTAuthority) -> Self {
        return Self { authority };
    }

    pub async fn listen(
        self,
        listen_addr: &str,
        tcp_timeout: Duration,
    ) -> Result<(), anyhow::Error> {
        let tcp = TcpListener::bind(listen_addr).await?;
        let udp = UdpSocket::bind(listen_addr).await?;
        let mut sf = ServerFuture::new(self.authority.catalog());

        sf.register_socket(udp);
        sf.register_listener(tcp, tcp_timeout);

        sf.block_until_done().await?;
        Ok(())
    }
}
