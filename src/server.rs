use std::time::Duration;
use tokio::net::{TcpListener, UdpSocket};

use trust_dns_server::server::ServerFuture;

use crate::authority::{init_catalog, TokioZTAuthority};

pub(crate) struct Server {
    zt: TokioZTAuthority,
}

impl Server {
    pub(crate) fn new(zt: TokioZTAuthority) -> Self {
        return Self { zt };
    }

    pub(crate) async fn listen(
        self,
        listen_addr: String,
        tcp_timeout: Duration,
    ) -> Result<(), anyhow::Error> {
        let tcp = TcpListener::bind(&listen_addr).await?;
        let udp = UdpSocket::bind(&listen_addr).await?;
        let mut sf = ServerFuture::new(init_catalog(self.zt).await?);

        sf.register_socket(udp);
        sf.register_listener(tcp, tcp_timeout);

        sf.block_until_done().await?;
        Ok(())
    }
}
