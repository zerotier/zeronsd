use crate::{authtoken_path, central_config, get_listen_ip, init_runtime};
use std::{
    sync::{Arc, Mutex},
    thread::sleep,
    time::Duration,
};
use tokio::runtime::Runtime;
use zerotier_central_api::{
    apis::configuration::Configuration,
    models::{Member, Network},
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
    let mut v: Vec<u8> = Vec::new();
    for _ in 0..len {
        v.push((rand::random::<u8>() % 26) + 'a' as u8);
    }

    // nasty
    v.into_iter()
        .map(|c| (c as char).to_string())
        .collect::<Vec<String>>()
        .join("")
}

fn network_definition() -> serde_json::Map<String, serde_json::Value> {
    let mut nd = serde_json::Map::default();
    let mut network_config = serde_json::Map::default();
    network_config.insert(
        String::from("name"),
        serde_json::Value::String(randstring(30)),
    );

    let mut assignment_pool = serde_json::Map::default();
    assignment_pool.insert(
        String::from("ipRangeStart"),
        serde_json::Value::String(String::from("172.16.240.1")),
    );
    assignment_pool.insert(
        String::from("ipRangeEnd"),
        serde_json::Value::String(String::from("172.16.240.254")),
    );

    network_config.insert(
        String::from("ipAssignmentPools"),
        serde_json::Value::Array(vec![serde_json::Value::Object(assignment_pool)]),
    );

    let mut routes = serde_json::Map::default();
    routes.insert(
        String::from("target"),
        serde_json::Value::String(String::from("172.16.240.0/24")),
    );

    network_config.insert(
        String::from("routes"),
        serde_json::Value::Array(vec![serde_json::Value::Object(routes)]),
    );

    let mut v4assign = serde_json::Map::default();
    v4assign.insert(String::from("zt"), serde_json::Value::Bool(true));
    network_config.insert(
        String::from("v4AssignMode"),
        serde_json::Value::Object(v4assign),
    );

    let mut v6assign = serde_json::Map::default();
    v6assign.insert(String::from("6plane"), serde_json::Value::Bool(false));
    network_config.insert(
        String::from("v6AssignMode"),
        serde_json::Value::Object(v6assign),
    );

    nd.insert(
        String::from("config"),
        serde_json::Value::Object(network_config),
    );

    nd
}

struct TestNetwork {
    network: Network,
    central: Configuration,
    zerotier: zerotier_one_api::apis::configuration::Configuration,
    runtime: Arc<Mutex<Runtime>>,
}

impl TestNetwork {
    fn new(runtime: Arc<Mutex<Runtime>>) -> Result<Self, anyhow::Error> {
        let authtoken = get_authtoken()?;

        let mut zerotier = zerotier_one_api::apis::configuration::Configuration::default();
        zerotier.api_key = Some(zerotier_one_api::apis::configuration::ApiKey {
            prefix: None,
            key: authtoken,
        });

        let identity = runtime
            .lock()
            .unwrap()
            .block_on(get_identity(&zerotier))
            .unwrap();

        let token = std::env::var("TOKEN").expect("Please provide TOKEN in the environment");
        let central = central_config(token);

        let network = runtime
            .lock()
            .unwrap()
            .block_on(zerotier_central_api::apis::network_api::new_network(
                &central,
                serde_json::Value::Object(network_definition()),
            ))
            .unwrap();

        let mut member = Member::new();
        member.node_id = Some(identity.clone());
        member.network_id = Some(network.clone().id.unwrap());

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

    let tmp = init_runtime();
    let runtime = std::sync::Arc::new(Mutex::new(tmp));
    let tn = TestNetwork::new(runtime.clone()).unwrap();
    let authtoken = authtoken_path(None);

    tn.join()?;

    let listen_ip = runtime
        .lock()
        .unwrap()
        .block_on(get_listen_ip(&authtoken, &tn.network.clone().id.unwrap()))?;
    eprintln!("My listen IP is {}", listen_ip);

    assert_ne!(listen_ip, String::from(""));

    Ok(())
}
