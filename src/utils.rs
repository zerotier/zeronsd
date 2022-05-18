use std::{net::IpAddr, path::Path, str::FromStr, sync::Once};

use ipnetwork::IpNetwork;
use reqwest::header::{HeaderMap, HeaderValue};
use tracing::warn;
use trust_dns_server::client::rr::{LowerName, Name};

use anyhow::anyhow;

use crate::traits::ToHostname;

// collections of test hosts files
pub const TEST_HOSTS_DIR: &str = "testdata/hosts-files";
// default domain parameter. FIXME change to home.arpa.
pub const DOMAIN_NAME: &str = "home.arpa.";
// zeronsd version calculated from Cargo.toml
pub const VERSION_STRING: &str = env!("CARGO_PKG_VERSION");
// address of Central
pub const CENTRAL_BASEURL: &str = "https://my.zerotier.com/api/v1";
// address of local zerotier instance
pub const ZEROTIER_LOCAL_URL: &str = "http://127.0.0.1:9993";

// this really needs to be replaced with lazy_static! magic
fn version() -> String {
    "zeronsd ".to_string() + VERSION_STRING
}

static LOGGER: Once = Once::new();

// initializes a logger
pub fn init_logger(level: Option<tracing::Level>) {
    LOGGER.call_once(|| {
        let loglevel = std::env::var("ZERONSD_LOG").or_else(|_| std::env::var("RUST_LOG"));

        let level = if let Ok(loglevel) = loglevel {
            crate::log::LevelFilter::from_str(&loglevel)
                .expect("invalid log level")
                .to_log()
        } else {
            level
        };

        tracing_log::log_tracer::LogTracer::init().expect("initializing logger failed");

        if let Some(level) = level {
            let subscriber = tracing_subscriber::FmtSubscriber::builder()
                // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
                // will be written to stdout.
                .with_max_level(level)
                // completes the builder.
                .finish();

            tracing::subscriber::set_global_default(subscriber)
                .expect("setting default subscriber failed");
        }
    })
}

// this provides the production configuration for talking to central through the openapi libraries.
pub fn central_client(token: String) -> Result<zerotier_central_api::Client, anyhow::Error> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "Authorization",
        HeaderValue::from_str(&format!("bearer {}", token))?,
    );

    Ok(zerotier_central_api::Client::new_with_client(
        &std::env::var("ZEROTIER_CENTRAL_INSTANCE").unwrap_or(CENTRAL_BASEURL.to_string()),
        reqwest::Client::builder()
            .user_agent(version())
            .https_only(true)
            .default_headers(headers)
            .build()?,
    ))
}

// extracts the ip from the CIDR. 10.0.0.1/32 becomes 10.0.0.1
pub fn parse_ip_from_cidr(ip_with_cidr: String) -> IpAddr {
    IpNetwork::from_str(&ip_with_cidr)
        .expect("Could not parse IP from CIDR")
        .ip()
}

// load and prepare the central API token
pub fn central_token(arg: Option<&Path>) -> Result<String, anyhow::Error> {
    if let Some(path) = arg {
        return Ok(std::fs::read_to_string(path)
            .expect("Could not load token file")
            .trim()
            .to_string());
    }

    if let Ok(token) = std::env::var("ZEROTIER_CENTRAL_TOKEN") {
        if token.len() > 0 {
            return Ok(token);
        }
    }

    return Err(anyhow!("missing zerotier central token: set ZEROTIER_CENTRAL_TOKEN in environment, or pass a file containing it with -t"));
}

// determine the path of the authtoken.secret
pub fn authtoken_path(arg: Option<&Path>) -> &Path {
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
pub fn domain_or_default(tld: Option<&str>) -> Result<Name, anyhow::Error> {
    if let Some(tld) = tld {
        if tld.len() > 0 {
            return Ok(Name::from_str(&format!("{}.", tld))?);
        } else {
            return Err(anyhow!("Domain name must not be empty if provided."));
        }
    };

    Ok(Name::from_str(DOMAIN_NAME)?)
}

// parse_member_name ensures member names are DNS compliant
pub fn parse_member_name(name: Option<String>, domain_name: Name) -> Option<Name> {
    if let Some(name) = name {
        let name = name.trim();
        if name.len() > 0 {
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

pub async fn get_member_name(
    authtoken_path: &Path,
    domain_name: Name,
) -> Result<LowerName, anyhow::Error> {
    let client = local_client_from_file(authtoken_path)?;

    let status = client.get_status().await?;
    if let Some(address) = &status.address {
        return Ok(("zt-".to_string() + address).to_fqdn(domain_name)?.into());
    }

    Err(anyhow!(
        "No member found for this instance; is zerotier connected to this network?"
    ))
}

fn local_client_from_file(
    authtoken_path: &Path,
) -> Result<zerotier_one_api::Client, anyhow::Error> {
    let authtoken = std::fs::read_to_string(authtoken_path)?;
    local_client(authtoken)
}

pub fn local_client(authtoken: String) -> Result<zerotier_one_api::Client, anyhow::Error> {
    let mut headers = HeaderMap::new();
    headers.insert("X-ZT1-Auth", HeaderValue::from_str(&authtoken)?);

    Ok(zerotier_one_api::Client::new_with_client(
        "http://127.0.0.1:9993",
        reqwest::Client::builder()
            .user_agent(version())
            .default_headers(headers)
            .build()?,
    ))
}

// get_listen_ips returns the IPs that the network is providing to the instance running zeronsd.
// 4193 and 6plane are handled up the stack.
pub async fn get_listen_ips(
    authtoken_path: &Path,
    network_id: &str,
) -> Result<Vec<String>, anyhow::Error> {
    let client = local_client_from_file(authtoken_path)?;

    match client.get_network(network_id).await {
        Err(error) => Err(anyhow!(
            "Error: {}. Are you joined to {}?",
            error,
            network_id
        )),
        Ok(listen) => {
            let assigned = listen.subtype_1.assigned_addresses.to_owned();
            if assigned.len() > 0 {
                Ok(assigned)
            } else {
                Err(anyhow!("No listen IPs available on this network"))
            }
        }
    }
}

// update_central_dns pushes the search records
pub async fn update_central_dns(
    domain_name: Name,
    ips: Vec<String>,
    client: zerotier_central_api::Client,
    network: String,
) -> Result<(), anyhow::Error> {
    let mut zt_network = client.get_network_by_id(&network).await?;

    let mut domain_name = domain_name;
    domain_name.set_fqdn(false);

    let dns = Some(zerotier_central_api::types::Dns {
        domain: Some(domain_name.to_string()),
        servers: Some(ips),
    });

    if let Some(mut zt_network_config) = zt_network.config.to_owned() {
        zt_network_config.dns = dns;
        zt_network.config = Some(zt_network_config);
        client.update_network(&network, &zt_network).await?;
    }

    Ok(())
}
