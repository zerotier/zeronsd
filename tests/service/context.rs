use zeronsd::utils::central_config;
use zerotier_central_api::{
    apis::configuration::Configuration,
    models::{Member, MemberConfig},
};

use super::{
    member::MemberUtil,
    utils::{get_authtoken, get_identity, zerotier_config},
};

// TestContext provides all the stuff we need to talk to run tests smoothly
#[derive(Clone)]
pub struct TestContext {
    pub member_config: Option<Box<MemberConfig>>,
    pub identity: String,
    pub zerotier: zerotier_one_api::apis::configuration::Configuration,
    pub central: Configuration,
}

impl TestContext {
    pub fn get_member(&mut self, network_id: String) -> Member {
        let mut member = Member::new();
        member.set_defaults(network_id, self.identity.clone());
        if self.member_config.is_some() {
            member.config = self.member_config.clone();
        }

        member
    }

    pub async fn default() -> Self {
        let authtoken = get_authtoken(None).expect("Could not read authtoken");
        let zerotier = zerotier_config(authtoken.clone());
        let identity = get_identity(&zerotier)
            .await
            .expect("Could not retrieve identity from zerotier");

        let token = std::env::var("TOKEN").expect("Please provide TOKEN in the environment");
        let central = central_config(token.clone());

        Self {
            member_config: None,
            identity,
            zerotier,
            central,
        }
    }
}
