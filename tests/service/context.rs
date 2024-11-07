use zeronsd::utils::{central_client, local_client, ZEROTIER_LOCAL_URL};

use super::{
    member::MemberUtil,
    utils::{get_authtoken, get_identity},
};

use zerotier_api::{central_api, service_api};

// TestContext provides all the stuff we need to talk to run tests smoothly
#[derive(Clone)]
pub struct TestContext {
    pub member_config: Option<central_api::types::MemberConfig>,
    pub identity: String,
    pub zerotier: service_api::Client,
    pub central: central_api::Client,
}

impl TestContext {
    pub fn get_member(&mut self, network_id: String) -> central_api::types::Member {
        let mut member = central_api::types::Member::new(network_id, self.identity.clone());
        if let cfg @ Some(_) = self.member_config.clone() {
            member.config = cfg;
        }

        member
    }

    pub async fn default() -> Self {
        let authtoken = get_authtoken(None).expect("Could not read authtoken");
        let zerotier = local_client(authtoken.clone(), ZEROTIER_LOCAL_URL.into()).unwrap();
        let identity = get_identity(&zerotier)
            .await
            .expect("Could not retrieve identity from zerotier");

        let token = std::env::var("TOKEN").expect("Please provide TOKEN in the environment");
        let central = central_client(token.clone()).unwrap();

        Self {
            member_config: None,
            identity,
            zerotier,
            central,
        }
    }
}
