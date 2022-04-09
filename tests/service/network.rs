use std::time::Duration;

use tracing::warn;
use zeronsd::utils::{authtoken_path, get_listen_ips};
use zerotier_central_api::{
    apis::configuration::Configuration,
    models::{Member, MemberConfig, Network},
};

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
        let mut mc = MemberConfig::new();
        mc.set_defaults(tc.identity.clone());
        mc.set_ip_assignments(ips);
        tc.member_config = Some(Box::new(mc));
        Self::new(network_def, tc).await
    }

    // constructor.
    pub async fn new(network_def: &str, tc: &mut TestContext) -> Result<Self, anyhow::Error> {
        let network = zerotier_central_api::apis::network_api::new_network(
            &tc.central,
            serde_json::Value::Object(network_definition(network_def.to_string())?),
        )
        .await
        .unwrap();

        let member = tc.get_member(network.clone().id.unwrap());

        zerotier_central_api::apis::network_member_api::update_network_member(
            &tc.central,
            &network.clone().id.unwrap(),
            &tc.identity,
            member.clone(),
        )
        .await
        .unwrap();

        let s = Self {
            network,
            member,
            context: tc.clone(),
        };

        s.join().await.unwrap();

        Ok(s)
    }

    // join zerotier-one to the test network
    pub async fn join(&self) -> Result<(), anyhow::Error> {
        let network = zerotier_one_api::models::Network::new();
        zerotier_one_api::apis::network_api::update_network(
            &self.context.zerotier,
            &self.network.id.clone().unwrap(),
            network,
        )
        .await?;

        let id = self.network.id.clone().unwrap();
        let mut count = 0;

        while let Err(e) = get_listen_ips(&authtoken_path(None), &id).await {
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
        Ok(zerotier_one_api::apis::network_api::delete_network(
            &self.context.zerotier,
            &self.network.id.clone().unwrap(),
        )
        .await?)
    }

    pub fn identity(&self) -> String {
        self.context.identity.clone()
    }

    pub fn central(&self) -> Configuration {
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
                zerotier_central_api::apis::network_api::delete_network(
                    &central,
                    &self.network.id.clone().unwrap(),
                )
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
