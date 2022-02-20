/// testing stuff; gated by feature flags. Does full round-trip processing of DNS results.
use crate::{
    addresses::Calculator,
    utils::{authtoken_path, central_config, get_listen_ips, init_logger, init_runtime},
};
use log::warn;
use std::{
    path::Path,
    sync::{Arc, Mutex},
    thread::sleep,
    time::Duration,
};
use tokio::runtime::Runtime;
use zerotier_central_api::{
    apis::configuration::Configuration,
    models::{Member, MemberConfig, Network},
};

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

// extract a network definiton from testdata. templates in a new name.
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

// returns the public identity of this instance of zerotier
pub(crate) async fn get_identity(
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

// unpack the authtoken based on what we're passed
pub(crate) fn get_authtoken(or: Option<&str>) -> Result<String, anyhow::Error> {
    Ok(std::fs::read_to_string(authtoken_path(
        or.map(|c| Path::new(c)),
    ))?)
}

// zerotier_config returns the openapi configuration required to talk to the local ztone instance
pub(crate) fn zerotier_config(
    authtoken: String,
) -> zerotier_one_api::apis::configuration::Configuration {
    let mut zerotier = zerotier_one_api::apis::configuration::Configuration::default();
    zerotier.api_key = Some(zerotier_one_api::apis::configuration::ApiKey {
        prefix: None,
        key: authtoken.clone(),
    });

    zerotier
}

// TestRuntime is a tokio runtime made for testing.
pub(crate) type TestRuntime = Arc<Mutex<Runtime>>;

pub(crate) fn init_test_runtime() -> TestRuntime {
    Arc::new(Mutex::new(init_runtime()))
}

// monkeypatches to Member
pub(crate) trait MemberUtil {
    // set some member defaults for testing
    fn set_defaults(&mut self, network_id: String, identity: String);
}

// monkeypatches to MemberConfig
pub(crate) trait MemberConfigUtil {
    fn set_ip_assignments(&mut self, ips: Vec<&str>);
    fn set_defaults(&mut self, identity: String);
}

impl MemberUtil for Member {
    fn set_defaults(&mut self, network_id: String, identity: String) {
        self.node_id = Some(identity.clone());
        self.network_id = Some(network_id);
        let mut mc = MemberConfig::new();
        mc.set_defaults(identity);
        self.config = Some(Box::new(mc));
    }
}

impl MemberConfigUtil for MemberConfig {
    fn set_ip_assignments(&mut self, ips: Vec<&str>) {
        self.ip_assignments = Some(ips.into_iter().map(|s| s.to_string()).collect())
    }

    fn set_defaults(&mut self, identity: String) {
        self.v_rev = None;
        self.v_major = None;
        self.v_proto = None;
        self.v_minor = None;
        self.tags = None;
        self.revision = None;
        self.no_auto_assign_ips = Some(false);
        self.last_authorized_time = None;
        self.last_deauthorized_time = None;
        self.id = None;
        self.creation_time = None;
        self.capabilities = None;
        self.ip_assignments = None;
        self.authorized = Some(true);
        self.active_bridge = None;
        self.identity = Some(identity);
    }
}

// TestContext provides all the stuff we need to talk to run tests smoothly
#[derive(Clone)]
pub(crate) struct TestContext {
    member_config: Option<Box<MemberConfig>>,
    identity: String,
    zerotier: zerotier_one_api::apis::configuration::Configuration,
    central: Configuration,
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
}

impl Default for TestContext {
    fn default() -> Self {
        let runtime = init_runtime();
        let authtoken = get_authtoken(None).expect("Could not read authtoken");
        let zerotier = zerotier_config(authtoken.clone());
        let identity = runtime
            .block_on(get_identity(&zerotier))
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

// TestNetwork creates a testnetwork in central and joins it. When this data is destroyed/dropped
// it will remove the network and leave it like nothing ever happened.
#[derive(Clone)]
pub(crate) struct TestNetwork {
    pub network: Network,
    runtime: Arc<Mutex<Runtime>>,
    context: TestContext,
    member: Member,
}

impl TestNetwork {
    // new_multi_ip covers situations where zeronsd is using more than one listening ip.
    pub fn new_multi_ip(
        runtime: TestRuntime,
        network_def: &str,
        tc: &mut TestContext,
        ips: Vec<&str>,
    ) -> Result<Self, anyhow::Error> {
        let mut mc = MemberConfig::new();
        mc.set_defaults(tc.identity.clone());
        mc.set_ip_assignments(ips);
        tc.member_config = Some(Box::new(mc));
        Self::new(runtime, network_def, tc)
    }

    // constructor.
    pub fn new(
        runtime: TestRuntime,
        network_def: &str,
        tc: &mut TestContext,
    ) -> Result<Self, anyhow::Error> {
        let network = runtime
            .lock()
            .unwrap()
            .block_on(zerotier_central_api::apis::network_api::new_network(
                &tc.central,
                serde_json::Value::Object(network_definition(network_def.to_string())?),
            ))
            .unwrap();

        let member = tc.get_member(network.clone().id.unwrap());

        runtime
            .lock()
            .unwrap()
            .block_on(
                zerotier_central_api::apis::network_member_api::update_network_member(
                    &tc.central,
                    &network.clone().id.unwrap(),
                    &tc.identity,
                    member.clone(),
                ),
            )
            .unwrap();

        let s = Self {
            network,
            member,
            runtime: runtime.clone(),
            context: tc.clone(),
        };

        s.join().unwrap();
        Ok(s)
    }

    // join zerotier-one to the test network
    pub fn join(&self) -> Result<(), anyhow::Error> {
        let network = zerotier_one_api::models::Network::new();
        self.runtime.lock().unwrap().block_on(
            zerotier_one_api::apis::network_api::update_network(
                &self.context.zerotier,
                &self.network.id.clone().unwrap(),
                network,
            ),
        )?;

        let id = self.network.id.clone().unwrap();
        let mut count = 0;

        while let Err(e) = self
            .runtime
            .lock()
            .unwrap()
            .block_on(get_listen_ips(&authtoken_path(None), &id))
        {
            sleep(Duration::new(1, 0));
            count += 1;
            if count >= 5 {
                warn!("5 attempts: While joining network: {:?}", e);
                count = 0;
            }
        }
        Ok(())
    }

    // leave the test network
    pub fn leave(&self) -> Result<(), anyhow::Error> {
        self.runtime.lock().unwrap().block_on(
            zerotier_one_api::apis::network_api::delete_network(
                &self.context.zerotier,
                &self.network.id.clone().unwrap(),
            ),
        )?;
        Ok(())
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
}

// drop just removes the network from central and leaves it. it tries to recover, not get more
// angry, in the face of errors.
impl Drop for TestNetwork {
    fn drop(&mut self) {
        let opt = self.network.id.clone();
        self.leave().unwrap_or(());
        self.runtime
            .lock()
            .unwrap()
            .block_on(zerotier_central_api::apis::network_api::delete_network(
                &self.context.central,
                &opt.unwrap(),
            ))
            .unwrap_or(());
    }
}

#[test]
fn test_get_listen_ip() -> Result<(), anyhow::Error> {
    init_logger();

    let tn = TestNetwork::new(
        init_test_runtime(),
        "basic-ipv4",
        &mut TestContext::default(),
    )
    .unwrap();
    let runtime = init_runtime();

    let listen_ips = runtime.block_on(get_listen_ips(
        &authtoken_path(None),
        &tn.network.clone().id.unwrap(),
    ))?;

    eprintln!("My listen IP is {}", listen_ips.first().unwrap());
    assert_ne!(*listen_ips.first().unwrap(), String::from(""));

    drop(tn);

    // see testdata/networks/basic-ipv4.json
    let mut ips = vec!["172.16.240.2", "172.16.240.3", "172.16.240.4"];
    let runtime = init_test_runtime();
    let tn = TestNetwork::new_multi_ip(
        runtime.clone(),
        "basic-ipv4",
        &mut TestContext::default(),
        ips.clone(),
    )
    .unwrap();

    let mut listen_ips = runtime.lock().unwrap().block_on(get_listen_ips(
        &authtoken_path(None),
        &tn.network.clone().id.unwrap(),
    ))?;

    assert_eq!(listen_ips.sort(), ips.sort());
    eprintln!("My listen IPs are {}", listen_ips.join(", "));

    let tn =
        TestNetwork::new(runtime.clone(), "rfc4193-only", &mut TestContext::default()).unwrap();

    let mut listen_ips = runtime.lock().unwrap().block_on(get_listen_ips(
        &authtoken_path(None),
        &tn.network.clone().id.unwrap(),
    ))?;

    let mut ips = vec![tn.member.clone().rfc4193()?.ip().to_string()];

    assert_eq!(listen_ips.sort(), ips.sort());
    eprintln!("My listen IPs are {}", listen_ips.join(", "));

    drop(tn);

    let tn = TestNetwork::new(runtime.clone(), "6plane-only", &mut TestContext::default()).unwrap();

    let mut listen_ips = runtime.lock().unwrap().block_on(get_listen_ips(
        &authtoken_path(None),
        &tn.network.clone().id.unwrap(),
    ))?;

    let mut ips = vec![tn.member.clone().sixplane()?.ip().to_string()];

    assert_eq!(listen_ips.sort(), ips.sort());
    eprintln!("My listen IPs are {}", listen_ips.join(", "));

    Ok(())
}
