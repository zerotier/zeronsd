use crate::{authtoken_path, central_config, get_listen_ip, init_runtime};
use std::{
    sync::{Arc, Mutex},
    thread::sleep,
    time::Duration,
};
use tokio::runtime::Runtime;
use zerotier_central_api::{
    apis::configuration::Configuration,
    models::{Member, MemberConfig, Network},
};

async fn get_identity(
    configuration: &zerotier_one_api::apis::configuration::Configuration,
) -> Result<String, anyhow::Error> {
    let status = zerotier_one_api::apis::status_api::get_status(configuration).await?;

    Ok(status
        .public_identity
        .unwrap()
        .splitn(3, ":")
        .nth(0)
        .unwrap()
        .to_owned())
}

fn get_authtoken() -> Result<String, anyhow::Error> {
    Ok(std::fs::read_to_string(authtoken_path(None))?)
}

fn randstring(len: u8) -> String {
    (0..len)
        .map(|_| (rand::random::<u8>() % 26) + 'a' as u8)
        .map(|c| {
            if rand::random::<bool>() {
                (c as char).to_ascii_uppercase()
            } else {
                c as char
            }
        })
        .map(|c| c.to_string())
        .collect::<Vec<String>>()
        .join("")
}

fn network_definition(
    name: String,
) -> Result<serde_json::Map<String, serde_json::Value>, anyhow::Error> {
    let mut res: serde_json::Map<String, serde_json::Value> = serde_json::from_reader(
        std::fs::File::open(format!("testdata/networks/{}.json", name))?,
    )?;

    if let serde_json::Value::Object(config) = res.clone().get("config").unwrap() {
        let mut new_config = config.clone();
        new_config.insert(
            "name".to_string(),
            serde_json::Value::String(randstring(30)),
        );

        res.insert("config".to_string(), serde_json::Value::Object(new_config));
    }

    Ok(res)
}

pub struct TestNetwork {
    pub network: Network,
    pub central: Configuration,
    pub zerotier: zerotier_one_api::apis::configuration::Configuration,
    pub authtoken: String,
    pub central_token: String,
    pub identity: String,
    runtime: Arc<Mutex<Runtime>>,
}

impl TestNetwork {
    pub fn new(network_def: &str) -> Result<Self, anyhow::Error> {
        let runtime = Arc::new(Mutex::new(init_runtime()));
        let authtoken = get_authtoken()?;

        let mut zerotier = zerotier_one_api::apis::configuration::Configuration::default();
        zerotier.api_key = Some(zerotier_one_api::apis::configuration::ApiKey {
            prefix: None,
            key: authtoken.clone(),
        });

        let identity = runtime
            .lock()
            .unwrap()
            .block_on(get_identity(&zerotier))
            .unwrap();

        let token = std::env::var("TOKEN").expect("Please provide TOKEN in the environment");
        let central = central_config(token.clone());

        let network = runtime
            .lock()
            .unwrap()
            .block_on(zerotier_central_api::apis::network_api::new_network(
                &central,
                serde_json::Value::Object(network_definition(network_def.to_string())?),
            ))
            .unwrap();

        let mut member = Member::new();
        member.node_id = Some(identity.clone());
        member.network_id = Some(network.clone().id.unwrap());
        member.config = Some(Box::new(MemberConfig {
            v_rev: None,
            v_major: None,
            v_proto: None,
            v_minor: None,
            tags: None,
            revision: None,
            no_auto_assign_ips: Some(false),
            last_authorized_time: None,
            last_deauthorized_time: None,
            id: None,
            creation_time: None,
            capabilities: None,
            ip_assignments: None,
            authorized: Some(true),
            active_bridge: None,
            identity: Some(identity.clone()),
        }));

        runtime
            .lock()
            .unwrap()
            .block_on(
                zerotier_central_api::apis::network_member_api::update_network_member(
                    &central,
                    &network.clone().id.unwrap(),
                    &identity,
                    member,
                ),
            )
            .unwrap();

        let s = Self {
            network,
            central,
            zerotier,
            identity,
            authtoken,
            central_token: token,
            runtime: runtime.clone(),
        };

        s.join().unwrap();
        Ok(s)
    }

    pub fn join(&self) -> Result<(), anyhow::Error> {
        let network = zerotier_one_api::models::Network::new();
        self.runtime.lock().unwrap().block_on(
            zerotier_one_api::apis::network_api::update_network(
                &self.zerotier,
                &self.network.id.clone().unwrap(),
                network,
            ),
        )?;

        let id = self.network.id.clone().unwrap();

        while let Err(e) = self
            .runtime
            .lock()
            .unwrap()
            .block_on(get_listen_ip(&authtoken_path(None), &id))
        {
            eprintln!("While joining network: {:?}", e);
            sleep(Duration::new(1, 0));
        }
        Ok(())
    }

    pub fn leave(&self) -> Result<(), anyhow::Error> {
        self.runtime.lock().unwrap().block_on(
            zerotier_one_api::apis::network_api::delete_network(
                &self.zerotier,
                &self.network.id.clone().unwrap(),
            ),
        )?;
        Ok(())
    }
}

impl Drop for TestNetwork {
    fn drop(&mut self) {
        let opt = self.network.id.clone();
        self.leave().unwrap();
        self.runtime
            .lock()
            .unwrap()
            .block_on(zerotier_central_api::apis::network_api::delete_network(
                &self.central,
                &opt.unwrap(),
            ))
            .unwrap();
    }
}

#[test]
#[ignore]
fn test_get_listen_ip() -> Result<(), anyhow::Error> {
    use crate::get_listen_ip;

    let tn = TestNetwork::new("basic-ipv4").unwrap();
    let runtime = init_runtime();

    let listen_ip = runtime.block_on(get_listen_ip(
        &authtoken_path(None),
        &tn.network.clone().id.unwrap(),
    ))?;

    eprintln!("My listen IP is {}", listen_ip);
    assert_ne!(listen_ip, String::from(""));

    Ok(())
}
