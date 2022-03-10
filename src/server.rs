use log::warn;
use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};
use tokio::{
    net::{TcpListener, UdpSocket},
    time::sleep,
};

use trust_dns_server::server::ServerFuture;

use crate::authority::{init_catalog, TokioZTAuthority};

#[derive(Clone)]
pub struct Server {
    zt: TokioZTAuthority,
}

impl Server {
    pub fn new(zt: TokioZTAuthority) -> Self {
        return Self { zt };
    }

    // listener routine for TCP and UDP.
    pub async fn listen(
        self,
        ip: IpAddr,
        tcp_timeout: Duration,
        certs: Option<Vec<rustls::Certificate>>,
        key: Option<rustls::PrivateKey>,
    ) -> Result<(), anyhow::Error> {
        loop {
            let sa = SocketAddr::new(ip, 53);
            let tcp = TcpListener::bind(sa).await?;
            let udp = UdpSocket::bind(sa).await?;

            let mut sf = ServerFuture::new(init_catalog(self.zt.clone()).await?);

            if certs.is_some() && key.is_some() {
                let tls = TcpListener::bind(SocketAddr::new(ip, 853)).await?;
                match sf.register_tls_listener(
                    tls,
                    tcp_timeout,
                    (certs.clone().unwrap(), key.clone().unwrap()),
                ) {
                    Ok(_) => {}
                    Err(e) => log::error!("Cannot start DoT listener: {}", e),
                }
            }

            sf.register_socket(udp);
            sf.register_listener(tcp, tcp_timeout);

            match sf.block_until_done().await {
                Ok(_) => return Ok(()),
                Err(e) => {
                    warn!(
                        "Error received: {}. Will attempt to restart listener in one second",
                        e
                    );
                    sleep(Duration::new(1, 0)).await;
                }
            }
        }
    }
}
