/*
 * Service abstraction provides a way to automatically generate services that are attached to
 * TestNetworks for testing against. Each Service is composed of a DNS service attached to a
 * TestNetwork. The service can then be resolved against, for example. Several parameters for
 * managing the underlying TestNetwork, and the Service are available via the ServiceConfig struct.
 */

use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::Path,
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
use ipnetwork::IpNetwork;
use lazy_static::lazy_static;
use log::{info, warn};
use rand::prelude::{IteratorRandom, SliceRandom};
use tokio::{
    sync::{Mutex, RwLock},
    task::JoinHandle,
};
use trust_dns_resolver::{
    config::{NameServerConfig, ResolverConfig, ResolverOpts},
    name_server::{GenericConnection, GenericConnectionProvider, TokioRuntime},
    AsyncResolver,
};

use zeronsd::{
    addresses::Calculator,
    authority::{find_members, init_trust_dns_authority, new_ptr_authority, ZTAuthority},
    server::Server,
    utils::{
        authtoken_path, central_config, domain_or_default, get_listen_ips, parse_ip_from_cidr,
    },
};

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
pub async fn get_identity(
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
pub fn get_authtoken(or: Option<&str>) -> Result<String, anyhow::Error> {
    Ok(std::fs::read_to_string(authtoken_path(
        or.map(|c| Path::new(c)),
    ))?)
}

// zerotier_config returns the openapi configuration required to talk to the local ztone instance
pub fn zerotier_config(authtoken: String) -> zerotier_one_api::apis::configuration::Configuration {
    let mut zerotier = zerotier_one_api::apis::configuration::Configuration::default();
    zerotier.api_key = Some(zerotier_one_api::apis::configuration::ApiKey {
        prefix: None,
        key: authtoken.clone(),
    });

    zerotier
}

// monkeypatches to Member
pub trait MemberUtil {
    // set some member defaults for testing
    fn set_defaults(&mut self, network_id: String, identity: String);
}

// monkeypatches to MemberConfig
pub trait MemberConfigUtil {
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
pub struct TestContext {
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

lazy_static! {
    static ref NETWORKS: Arc<Mutex<HashMap<String, TestNetwork>>> =
        Arc::new(Mutex::new(HashMap::new()));
}

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
        let identity = self.identity();
        tokio::task::block_in_place(move || {
            tokio::runtime::Handle::current().block_on(async {
                if let Some(network) = NETWORKS.lock().await.remove(&identity) {
                    network.leave().await.unwrap();
                    let central = network.central();
                    zerotier_central_api::apis::network_api::delete_network(&central, &identity)
                        .await
                        .unwrap();
                }
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

type SocketVec = Vec<SocketAddr>;

pub trait ToIPv4Vec {
    fn to_ipv4_vec(self) -> Vec<Ipv4Addr>;
}

pub trait ToIPv6Vec {
    fn to_ipv6_vec(self) -> Vec<Ipv6Addr>;
}

pub trait ToPTRVec {
    fn to_ptr_vec(self) -> Vec<String>;
}

impl ToIPv4Vec for SocketVec {
    fn to_ipv4_vec(self) -> Vec<Ipv4Addr> {
        self.into_iter()
            .filter_map(|ip| match ip.ip() {
                IpAddr::V4(ip) => Some(ip),
                IpAddr::V6(_) => None,
            })
            .collect::<Vec<Ipv4Addr>>()
    }
}

impl ToIPv6Vec for SocketVec {
    fn to_ipv6_vec(self) -> Vec<Ipv6Addr> {
        self.into_iter()
            .filter_map(|ip| match ip.ip() {
                IpAddr::V4(_) => None,
                IpAddr::V6(ip) => Some(ip),
            })
            .collect::<Vec<Ipv6Addr>>()
    }
}

impl ToPTRVec for SocketVec {
    fn to_ptr_vec(self) -> Vec<String> {
        self.into_iter()
            .map(|ip| ip.ip().to_string())
            .collect::<Vec<String>>()
    }
}

lazy_static! {
    static ref SERVERS: Arc<Mutex<Vec<JoinHandle<Result<(), anyhow::Error>>>>> =
        Arc::new(Mutex::new(Vec::new()));
    static ref AUTHORITY_HANDLE: Arc<Mutex<Option<JoinHandle<()>>>> = Arc::new(Mutex::new(None));
}

#[derive(Clone)]
pub struct Service {
    tn: Arc<TestNetwork>,
    resolvers: Resolvers,
    update_interval: Option<Duration>,
    pub listen_ips: Vec<SocketAddr>,
}

#[async_trait]
pub trait Lookup {
    async fn lookup_a(&self, record: String) -> Vec<Ipv4Addr>;
    async fn lookup_aaaa(&self, record: String) -> Vec<Ipv6Addr>;
    async fn lookup_ptr(&self, record: String) -> Vec<String>;
}

#[async_trait]
impl Lookup for Resolver {
    async fn lookup_a(&self, record: String) -> Vec<Ipv4Addr> {
        self.ipv4_lookup(record)
            .await
            .unwrap()
            .as_lookup()
            .record_iter()
            .map(|r| r.rdata().clone().into_a().unwrap())
            .collect()
    }

    async fn lookup_aaaa(&self, record: String) -> Vec<Ipv6Addr> {
        self.ipv6_lookup(record)
            .await
            .unwrap()
            .as_lookup()
            .record_iter()
            .map(|r| r.rdata().clone().into_aaaa().unwrap())
            .collect()
    }

    async fn lookup_ptr(&self, record: String) -> Vec<String> {
        self.reverse_lookup(record.parse().unwrap())
            .await
            .unwrap()
            .as_lookup()
            .record_iter()
            .map(|r| r.rdata().clone().into_ptr().unwrap().to_string())
            .collect()
    }
}

pub enum HostsType {
    Path(&'static str),
    Fixture(&'static str),
    None,
}

async fn create_listeners(
    tn: &TestNetwork,
    hosts: HostsType,
    update_interval: Option<Duration>,
    wildcard_everything: bool,
) -> (
    Vec<SocketAddr>,
    JoinHandle<()>,
    Vec<JoinHandle<Result<(), anyhow::Error>>>,
) {
    let listen_cidrs = get_listen_ips(&authtoken_path(None), &tn.network.clone().id.unwrap())
        .await
        .unwrap();

    let mut listen_ips = Vec::new();

    let mut ipmap = HashMap::new();
    let mut authority_map = HashMap::new();
    let authority = init_trust_dns_authority(domain_or_default(None).unwrap());

    for cidr in listen_cidrs.clone() {
        let listen_ip = parse_ip_from_cidr(cidr.clone());
        let socket_addr = SocketAddr::new(listen_ip.clone(), 53);
        listen_ips.push(socket_addr);
        let cidr = IpNetwork::from_str(&cidr.clone()).unwrap();
        if !ipmap.contains_key(&listen_ip) {
            ipmap.insert(listen_ip, cidr.network());
        }

        if !authority_map.contains_key(&cidr) {
            let ptr_authority = new_ptr_authority(cidr).unwrap();
            authority_map.insert(cidr, ptr_authority.clone());
        }
    }

    if let Some(v6assign) = tn.network.config.clone().unwrap().v6_assign_mode {
        if v6assign.rfc4193.unwrap_or(false) {
            let cidr = tn.network.clone().rfc4193().unwrap();
            if !authority_map.contains_key(&cidr) {
                let ptr_authority = new_ptr_authority(cidr).unwrap();
                authority_map.insert(cidr, ptr_authority);
            }
        }
    }

    let update_interval = update_interval.unwrap_or(Duration::new(1, 0));

    let mut ztauthority = ZTAuthority::new(
        domain_or_default(None).unwrap(),
        tn.network.clone().id.unwrap(),
        tn.central(),
        match hosts {
            HostsType::Fixture(hosts) => Some(
                Path::new(&format!("{}/{}", zeronsd::utils::TEST_HOSTS_DIR, hosts)).to_path_buf(),
            ),
            HostsType::Path(hosts) => Some(Path::new(hosts).to_path_buf()),
            HostsType::None => None,
        },
        authority_map,
        update_interval,
        authority.clone(),
    );

    if wildcard_everything {
        ztauthority.wildcard_everything();
    }

    let arc_authority = Arc::new(RwLock::new(ztauthority));
    let authority_handle = tokio::spawn(find_members(arc_authority.clone()));
    let mut servers = Vec::new();

    tokio::time::sleep(Duration::new(1, 0)).await;

    for ip in listen_ips.clone() {
        let server = Server::new(arc_authority.to_owned());
        info!("Serving {}", ip.clone());
        servers.push(tokio::spawn(server.listen(ip, Duration::new(0, 500))));
    }

    (listen_ips, authority_handle, servers)
}

type Resolver = AsyncResolver<GenericConnection, GenericConnectionProvider<TokioRuntime>>;

type Resolvers = Vec<Arc<Resolver>>;

fn create_resolvers(sockets: Vec<SocketAddr>) -> Resolvers {
    let mut resolvers = Vec::new();

    for socket in sockets {
        let mut resolver_config = ResolverConfig::new();
        resolver_config.add_search(domain_or_default(None).unwrap());
        resolver_config.add_name_server(NameServerConfig {
            socket_addr: socket,
            protocol: trust_dns_resolver::config::Protocol::Udp,
            tls_dns_name: None,
            trust_nx_responses: true,
        });

        let mut opts = ResolverOpts::default();
        opts.cache_size = 0;
        opts.rotate = true;
        opts.use_hosts_file = false;
        opts.positive_min_ttl = Some(Duration::new(0, 0));
        opts.positive_max_ttl = Some(Duration::new(0, 0));
        opts.negative_min_ttl = Some(Duration::new(0, 0));
        opts.negative_max_ttl = Some(Duration::new(0, 0));

        resolvers.push(Arc::new(
            trust_dns_resolver::TokioAsyncResolver::tokio(resolver_config, opts).unwrap(),
        ));
    }

    resolvers
}

pub struct ServiceConfig {
    hosts: HostsType,
    update_interval: Option<Duration>,
    ips: Option<Vec<&'static str>>,
    wildcard_everything: bool,
    network_filename: Option<&'static str>,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            network_filename: None,
            hosts: HostsType::None,
            update_interval: None,
            ips: None,
            wildcard_everything: false,
        }
    }
}

impl ServiceConfig {
    pub fn network_filename(mut self, n: &'static str) -> Self {
        self.network_filename = Some(n);
        self
    }

    pub fn hosts(mut self, h: HostsType) -> Self {
        self.hosts = h;
        self
    }

    pub fn update_interval(mut self, u: Option<Duration>) -> Self {
        self.update_interval = u;
        self
    }

    pub fn ips(mut self, ips: Option<Vec<&'static str>>) -> Self {
        self.ips = ips;
        self
    }

    pub fn wildcard_everything(mut self, w: bool) -> Self {
        self.wildcard_everything = w;
        self
    }
}

impl Service {
    pub async fn new(sc: ServiceConfig) -> Self {
        let network_filename = sc.network_filename.unwrap_or("basic-ipv4");
        let tn = if let Some(ips) = sc.ips {
            TestNetwork::new_multi_ip(network_filename, &mut TestContext::default().await, ips)
                .await
                .unwrap()
        } else {
            TestNetwork::new(network_filename, &mut TestContext::default().await)
                .await
                .unwrap()
        };

        let (listen_ips, authority_handle, servers) =
            create_listeners(&tn, sc.hosts, sc.update_interval, sc.wildcard_everything).await;

        let mut lock = SERVERS.lock().await;

        for server in servers {
            lock.push(server);
        }

        let mut lock = AUTHORITY_HANDLE.lock().await;
        lock.replace(authority_handle);

        Self {
            tn: Arc::new(tn),
            resolvers: create_resolvers(listen_ips.clone()),
            listen_ips,
            update_interval: sc.update_interval,
        }
    }

    pub fn any_listen_ip(self) -> IpAddr {
        self.listen_ips
            .clone()
            .into_iter()
            .choose(&mut rand::thread_rng())
            .unwrap()
            .clone()
            .ip()
    }

    pub fn network(&self) -> Arc<TestNetwork> {
        self.tn.clone()
    }

    pub fn resolvers(&self) -> Resolvers {
        self.resolvers.clone()
    }

    pub fn any_resolver(&self) -> Arc<Resolver> {
        self.resolvers()
            .choose(&mut rand::thread_rng())
            .to_owned()
            .unwrap()
            .clone()
    }

    pub fn member_record(&self) -> String {
        format!("zt-{}.domain.", self.network().identity().clone())
    }

    pub async fn change_name(&self, name: &'static str) {
        let mut member = zerotier_central_api::apis::network_member_api::get_network_member(
            &self.network().central(),
            &self.network().network.clone().id.unwrap(),
            &self.network().identity(),
        )
        .await
        .unwrap();

        member.name = Some(name.to_string());

        zerotier_central_api::apis::network_member_api::update_network_member(
            &self.network().central(),
            &self.network().network.clone().id.unwrap(),
            &self.network().identity(),
            member,
        )
        .await
        .unwrap();

        if self.update_interval.is_some() {
            tokio::time::sleep(self.update_interval.unwrap()).await; // wait for it to update
        }
    }

    pub fn test_network(&self) -> Arc<TestNetwork> {
        self.tn.clone()
    }
}

#[async_trait]
impl Lookup for Service {
    async fn lookup_a(&self, record: String) -> Vec<Ipv4Addr> {
        self.any_resolver().lookup_a(record).await
    }

    async fn lookup_aaaa(&self, record: String) -> Vec<Ipv6Addr> {
        self.any_resolver().lookup_aaaa(record).await
    }

    async fn lookup_ptr(&self, record: String) -> Vec<String> {
        self.any_resolver().lookup_ptr(record).await
    }
}
