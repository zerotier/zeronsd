use std::time::Duration;
use tokio::net::{TcpListener, UdpSocket};

use central::apis::configuration::Configuration;
use trust_dns_server::{authority::Catalog, server::ServerFuture};

pub struct Server {
    catalog: Catalog,
    _config: Configuration,
    _network: String,
}

impl Server {
    pub fn new(catalog: Catalog, _config: Configuration, _network: String) -> Self {
        return Self {
            catalog,
            _config,
            _network,
        };
    }

    pub async fn listen(
        self,
        listen_addr: &str,
        tcp_timeout: Duration,
    ) -> Result<(), anyhow::Error> {
        let tcp = TcpListener::bind(listen_addr).await?;
        let udp = UdpSocket::bind(listen_addr).await?;
        let mut sf = ServerFuture::new(self.catalog);

        sf.register_socket(udp);
        sf.register_listener(tcp, tcp_timeout);

        sf.block_until_done().await?;
        Ok(())
    }
}
