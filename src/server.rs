use log::warn;
use std::time::Duration;
use tokio::{
    net::{TcpListener, UdpSocket},
    time::sleep,
};

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
        loop {
            let tcp = TcpListener::bind(&listen_addr).await?;
            let udp = UdpSocket::bind(&listen_addr).await?;
            let mut sf = ServerFuture::new(init_catalog(self.zt.clone()).await?);

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
