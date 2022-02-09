use std::{net::IpAddr, path::Path, str::FromStr};

use ipnetwork::IpNetwork;
use log::warn;
use regex::Regex;
use tokio::runtime::Runtime;
use trust_dns_resolver::IntoName;
use trust_dns_server::client::rr::Name;
use zerotier_central_api::apis::configuration::Configuration;

use anyhow::anyhow;

// default domain parameter. FIXME change to home.arpa.
pub(crate) const DOMAIN_NAME: &str = "domain.";
// zeronsd version calculated from Cargo.toml
pub(crate) const VERSION_STRING: &str = env!("CARGO_PKG_VERSION");

// this really needs to be replaced with lazy_static! magic
fn version() -> String {
    "zeronsd ".to_string() + VERSION_STRING
}

// this provides the production configuration for talking to central through the openapi libraries.
pub(crate) fn central_config(token: String) -> Configuration {
    let mut config = Configuration {
        user_agent: Some(version()),
        bearer_access_token: Some(token),
        ..Default::default()
    };

    if let Ok(instance) = std::env::var("ZEROTIER_CENTRAL_INSTANCE") {
        config.base_path = instance;
    }

    config
}

// create a tokio runtime. We don't use the macros (they are hard to use) so this is the closest
// we'll get to being able to get a runtime easily.
pub(crate) fn init_runtime() -> Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(num_cpus::get())
        .thread_name("zeronsd")
        .build()
        .expect("failed to initialize tokio")
}

// extracts the ip from the CIDR. 10.0.0.1/32 becomes 10.0.0.1
pub(crate) fn parse_ip_from_cidr(ip_with_cidr: String) -> IpAddr {
    IpNetwork::from_str(&ip_with_cidr)
        .expect("Could not parse IP from CIDR")
        .ip()
}

// load and prepare the central API token
pub(crate) fn central_token(arg: Option<&Path>) -> Result<String, anyhow::Error> {
    if let Some(path) = arg {
        return Ok(std::fs::read_to_string(path)
            .expect("Could not load token file")
            .trim()
            .to_string());
    }

    if let Ok(token) = std::env::var("ZEROTIER_CENTRAL_TOKEN") {
        if !token.is_empty() {
            return Ok(token);
        }
    }

    return Err(anyhow!("missing zerotier central token: set ZEROTIER_CENTRAL_TOKEN in environment, or pass a file containing it with -t"));
}

// determine the path of the authtoken.secret
pub(crate) fn authtoken_path(arg: Option<&Path>) -> &Path {
    if let Some(arg) = arg {
        return arg;
    }

    if cfg!(target_os = "linux") {
        Path::new("/var/lib/zerotier-one/authtoken.secret")
    } else if cfg!(target_os = "windows") {
        Path::new("C:/ProgramData/ZeroTier/One/authtoken.secret")
    } else if cfg!(target_os = "macos") {
        Path::new("/Library/Application Support/ZeroTier/One/authtoken.secret")
    } else {
        panic!("authtoken.secret not found; please provide the -s option to provide a custom path")
    }
}

// use the default tld if none is supplied.
pub(crate) fn domain_or_default(tld: Option<&str>) -> Result<Name, anyhow::Error> {
    if let Some(tld) = tld {
        if tld.is_empty() {
            return Err(anyhow!("Domain name must not be empty if provided."));
        }
        return Ok(Name::from_str(&format!("{}.", tld))?);
    };

    Ok(Name::from_str(DOMAIN_NAME)?)
}

// parse_member_name ensures member names are DNS compliant
pub(crate) fn parse_member_name(name: Option<String>, domain_name: Name) -> Option<Name> {
    if let Some(name) = name {
        let name = name.trim();
        if !name.is_empty() {
            match name.to_fqdn(domain_name) {
                Ok(record) => return Some(record),
                Err(e) => {
                    warn!("Record {} not entered into catalog: {:?}", name, e);
                    return None;
                }
            };
        }
    }

    None
}

// get_listen_ips returns the IPs that the network is providing to the instance running zeronsd.
// 4193 and 6plane are handled up the stack.
pub(crate) async fn get_listen_ips(
    authtoken_path: &Path,
    network_id: &str,
) -> Result<Vec<String>, anyhow::Error> {
    let authtoken = std::fs::read_to_string(authtoken_path)?;
    let mut configuration = zerotier_one_api::apis::configuration::Configuration::default();
    let api_key = zerotier_one_api::apis::configuration::ApiKey {
        prefix: None,
        key: authtoken,
    };

    configuration.user_agent = Some(version());
    configuration.api_key = Some(api_key);

    match zerotier_one_api::apis::network_api::get_network(&configuration, network_id).await {
        Err(error) => {
            match error {
                zerotier_one_api::apis::Error::ResponseError(_) => {
                    Err(anyhow!("Are you joined to {}?", network_id))
                }
                zerotier_one_api::apis::Error::Reqwest(_) => Err(anyhow!(
                    "Can't connect to zerotier-one at {:}. Is it installed and running?",
                    configuration.base_path
                )),
                // TODO ERROR - error in response: status code 403 Forbidden (wrong authtoken)
                other_error => Err(anyhow!(other_error)),
            }
        }
        Ok(listen) => {
            if let Some(assigned) = listen.assigned_addresses {
                if !assigned.is_empty() {
                    return Ok(assigned);
                }
            }
            Err(anyhow!("No listen IPs available on this network"))
        }
    }
}

// update_central_dns pushes the search records
pub(crate) fn update_central_dns(
    runtime: &mut Runtime,
    domain_name: Name,
    ips: Vec<String>,
    config: Configuration,
    network: String,
) -> Result<(), anyhow::Error> {
    let mut zt_network = runtime.block_on(
        zerotier_central_api::apis::network_api::get_network_by_id(&config, &network),
    )?;

    let mut domain_name = domain_name.clone();
    domain_name.set_fqdn(false);

    let dns = Some(Box::new(zerotier_central_api::models::NetworkConfigDns {
        domain: Some(domain_name.to_string()),
        servers: Some(ips),
    }));

    if let Some(mut zt_network_config) = zt_network.config.to_owned() {
        zt_network_config.dns = dns;
        zt_network.config = Some(zt_network_config);
        runtime.block_on(zerotier_central_api::apis::network_api::update_network(
            &config, &network, zt_network,
        ))?;
    }

    Ok(())
}

// translation_table should also be lazy_static and provides a small match set to find and correct
// problems with member namesl.
fn translation_table() -> Vec<(Regex, &'static str)> {
    vec![
        (Regex::new(r"\s+").unwrap(), "-"), // translate whitespace to `-`
        (Regex::new(r"[^.\s\w\d-]+").unwrap(), ""), // catch-all at the end
    ]
}

pub(crate) trait ToHostname {
    fn to_hostname(self) -> Result<Name, anyhow::Error>;
    fn to_fqdn(self, domain: Name) -> Result<Name, anyhow::Error>;
}

impl ToHostname for &str {
    fn to_hostname(self) -> Result<Name, anyhow::Error> {
        self.clone().to_string().to_hostname()
    }

    fn to_fqdn(self, domain: Name) -> Result<Name, anyhow::Error> {
        Ok(self.to_hostname()?.append_domain(&domain))
    }
}

impl ToHostname for String {
    // to_hostname turns member names into trust-dns compatible dns names.
    fn to_hostname(self) -> Result<Name, anyhow::Error> {
        let mut s = self.clone().trim().to_string();
        for (regex, replacement) in translation_table() {
            s = regex.replace_all(&s, replacement).to_string();
        }

        let s = s.trim();

        if s == "." || s.ends_with(".") {
            return Err(anyhow!("Record {} not entered into catalog: '.' and records that ends in '.' are disallowed", s));
        }

        if s.is_empty() {
            return Err(anyhow!("translated hostname {} is an empty string", self));
        }

        Ok(s.trim().into_name()?)
    }

    fn to_fqdn(self, domain: Name) -> Result<Name, anyhow::Error> {
        Ok(self.to_hostname()?.append_domain(&domain))
    }
}
