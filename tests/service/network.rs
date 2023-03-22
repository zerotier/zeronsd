use std::time::Duration;

use tracing::warn;
use zeronsd::utils::{authtoken_path, get_listen_ips, ZEROTIER_LOCAL_URL};
use zerotier_central_api::types::{Member, MemberConfig, Network};
use zerotier_one_api::types::{NetworkSubtype0, NetworkSubtype1};

use super::{context::TestContext, member::MemberConfigUtil, utils::network_definition};

// TestNetwork creates a testnetwork in central and joins it. When this data is destroyed/dropped
// it will remove the network and leave it like nothing ever happened.
#[derive(Clone)]
pub struct TestNetwork {
    pub network: Network,
    context: TestContext,
    member: Member,
}

impl TestNetwork {
    // new_multi_ip covers situations where zeronsd is using more than one listening ip.
    pub async fn new_multi_ip(
        network_def: &str,
        tc: &mut TestContext,
        ips: Vec<&str>,
    ) -> Result<Self, anyhow::Error> {
        let mut mc = MemberConfig::new(tc.identity.clone());
        mc.set_ip_assignments(ips);
        tc.member_config = Some(mc);
        Self::new(network_def, tc).await
    }

    // constructor.
    pub async fn new(network_def: &str, tc: &mut TestContext) -> Result<Self, anyhow::Error> {
        let network = tc
            .central
            .new_network(&network_definition(network_def.to_string())?)
            .await
            .unwrap();

        let member = tc.get_member(network.clone().id.unwrap());

        tc.central
            .update_network_member(&network.clone().id.unwrap(), &tc.identity, &member)
            .await
            .unwrap();

        let s = Self {
            network: network.to_owned(),
            member,
            context: tc.clone(),
        };

        s.join().await.unwrap();

        Ok(s)
    }

    // join zerotier-one to the test network
    pub async fn join(&self) -> Result<(), anyhow::Error> {
        let network = zerotier_one_api::types::Network {
            subtype_0: NetworkSubtype0 {
                allow_dns: Some(true),
                allow_global: Some(false),
                allow_default: Some(false),
                allow_managed: Some(true),
            },
            subtype_1: NetworkSubtype1 {
                status: None,
                type_: None,
                routes: Vec::new(),
                port_error: None,
                port_device_name: None,
                netconf_revision: None,
                name: None,
                multicast_subscriptions: Vec::new(),
                mtu: None,
                mac: None,
                id: None,
                dns: None,
                broadcast_enabled: None,
                bridge: None,
                assigned_addresses: Vec::new(),
                allow_dns: Some(true),
                allow_global: Some(false),
                allow_default: Some(false),
                allow_managed: Some(true),
            },
        };

        self.context
            .zerotier
            .update_network(&self.network.id.clone().unwrap(), &network)
            .await?;

        let id = self.network.id.clone().unwrap();
        let mut count = 0;

        while let Err(e) = get_listen_ips(&authtoken_path(None), &id, ZEROTIER_LOCAL_URL.into()).await {
            tokio::time::sleep(Duration::new(1, 0)).await;
            count += 1;
            if count >= 5 {
                warn!("5 attempts: While joining network: {:?}", e);
                count = 0;
            }
        }
        Ok(())
    }

    // leave the test network
    pub async fn leave(&self) -> Result<(), anyhow::Error> {
        Ok(self
            .context
            .zerotier
            .delete_network(&self.network.id.clone().unwrap())
            .await?
            .to_owned())
    }

    pub fn identity(&self) -> String {
        self.context.identity.clone()
    }

    pub fn central(&self) -> zerotier_central_api::Client {
        self.context.central.clone()
    }

    pub fn member(&self) -> Member {
        self.member.clone()
    }

    pub fn teardown(&mut self) {
        tokio::task::block_in_place(move || {
            tokio::runtime::Handle::current().block_on(async {
                self.leave().await.unwrap();
                let central = self.central();
                central
                    .delete_network(&self.network.id.clone().unwrap())
                    .await
                    .unwrap();
            })
        })
    }
}

// drop just removes the network from central and leaves it. it tries to recover, not get more
// angry, in the face of errors.
impl Drop for TestNetwork {
    fn drop(&mut self) {
        self.teardown()
    }
}
