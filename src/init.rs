use std::{
    collections::{hash_map::Entry, HashMap},
    path::PathBuf,
    str::FromStr,
    time::Duration,
};

use anyhow::anyhow;
use ipnetwork::IpNetwork;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use openssl::{pkey::PKey, stack::Stack, x509::X509};

use crate::{
    addresses::*,
    authority::{find_members, RecordAuthority, ZTAuthority},
    server::*,
    traits::ToPointerSOA,
    utils::*,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Launcher {
    pub domain: Option<String>,
    pub hosts: Option<PathBuf>,
    pub secret: Option<PathBuf>,
    pub token: Option<PathBuf>,
    pub chain_cert: Option<PathBuf>,
    pub tls_cert: Option<PathBuf>,
    pub tls_key: Option<PathBuf>,
    pub wildcard: bool,
    pub log_level: Option<crate::log::LevelFilter>,
    pub local_url: String,
    #[serde(skip_deserializing)]
    pub network_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum ConfigFormat {
    JSON,
    YAML,
    TOML,
}

impl FromStr for ConfigFormat {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "json" | "JSON" => Ok(ConfigFormat::JSON),
            "yaml" | "YAML" => Ok(ConfigFormat::YAML),
            "toml" | "TOML" => Ok(ConfigFormat::TOML),
            _ => Err(anyhow!(
                "invalid format: allowed values: [json, yaml, toml]"
            )),
        }
    }
}

impl Default for Launcher {
    fn default() -> Self {
        Launcher {
            domain: None,
            hosts: None,
            secret: None,
            token: None,
            chain_cert: None,
            tls_cert: None,
            tls_key: None,
            wildcard: false,
            network_id: None,
            log_level: None,
            local_url: ZEROTIER_LOCAL_URL.to_string(),
        }
    }
}

impl Launcher {
    pub fn new_from_config(filename: &str, format: ConfigFormat) -> Result<Self, anyhow::Error> {
        let res = std::fs::read_to_string(filename)?;
        Self::parse_format(&res, format)
    }

    pub fn parse_format(s: &str, format: ConfigFormat) -> Result<Self, anyhow::Error> {
        Ok(match format {
            ConfigFormat::JSON => serde_json::from_str(s)?,
            ConfigFormat::YAML => serde_yaml::from_str(s)?,
            ConfigFormat::TOML => toml::from_str(s)?,
        })
    }

    pub fn parse(s: &str, network_id: String, format: ConfigFormat) -> Result<Self, anyhow::Error> {
        let mut l: Launcher = Self::parse_format(s, format)?;
        l.network_id = Some(network_id);
        Ok(l)
    }

    pub async fn start(&self) -> Result<ZTAuthority, anyhow::Error> {
        crate::utils::init_logger(
            self.log_level
                .clone()
                .unwrap_or(crate::log::LevelFilter::Info)
                .to_log(),
        );

        if self.network_id.is_none() {
            return Err(anyhow!("network ID is invalid; cannot continue"));
        }

        let domain_name = domain_or_default(self.domain.as_deref())?;
        let authtoken = authtoken_path(self.secret.as_deref());
        let client = central_client(central_token(self.token.as_deref())?)?;

        info!("Welcome to ZeroNS!");
        let ips = get_listen_ips(
            &authtoken,
            &self.network_id.clone().unwrap(),
            self.local_url.clone(),
        )
        .await?;

        // more or less the setup for the "main loop"
        if !ips.is_empty() {
            update_central_dns(
                domain_name.clone(),
                ips.iter()
                    .map(|i| parse_ip_from_cidr(i.clone()).to_string())
                    .collect(),
                client.clone(),
                self.network_id.clone().unwrap(),
            )
            .await?;

            let mut listen_ips = Vec::new();
            let mut ipmap = HashMap::new();
            let mut authority_map = HashMap::new();

            for cidr in ips.clone() {
                let listen_ip = parse_ip_from_cidr(cidr.clone());
                listen_ips.push(listen_ip);
                let cidr = IpNetwork::from_str(&cidr.clone())?;
                ipmap.entry(listen_ip).or_insert_with(|| cidr.network());

                if let Entry::Vacant(e) = authority_map.entry(cidr) {
                    tracing::debug!("{}", cidr.to_ptr_soa_name()?);
                    let ptr_authority =
                        RecordAuthority::new(cidr.to_ptr_soa_name()?, cidr.to_ptr_soa_name()?)
                            .await?;
                    e.insert(ptr_authority);
                }
            }

            let member_name =
                get_member_name(authtoken, domain_name.clone(), self.local_url.clone()).await?;

            let network = client
                .get_network_by_id(&self.network_id.clone().unwrap())
                .await?;

            if let Some(v6assign) = network.config.clone().unwrap().v6_assign_mode {
                if v6assign._6plane.unwrap_or(false) {
                    warn!("6PLANE PTR records are not yet supported");
                }

                if v6assign.rfc4193.unwrap_or(false) {
                    let cidr = network.clone().rfc4193().unwrap();
                    if let Entry::Vacant(e) = authority_map.entry(cidr) {
                        tracing::debug!("{}", cidr.to_ptr_soa_name()?);
                        let ptr_authority =
                            RecordAuthority::new(cidr.to_ptr_soa_name()?, cidr.to_ptr_soa_name()?)
                                .await?;
                        e.insert(ptr_authority);
                    }
                }
            }

            let authority =
                RecordAuthority::new(domain_name.clone().into(), member_name.clone()).await?;

            let ztauthority = ZTAuthority {
                client,
                network_id: self.network_id.clone().unwrap(),
                hosts: None, // this will be parsed later.
                hosts_file: self.hosts.clone(),
                reverse_authority_map: authority_map,
                forward_authority: authority,
                wildcard: self.wildcard,
                update_interval: Duration::new(30, 0),
            };

            tokio::spawn(find_members(ztauthority.clone()));

            let server = Server::new(ztauthority.to_owned());
            for ip in listen_ips {
                info!("Your IP for this network: {}", ip);

                let tls_cert = if let Some(tls_cert) = self.tls_cert.clone() {
                    let pem = std::fs::read(tls_cert)?;
                    Some(X509::from_pem(&pem)?)
                } else {
                    None
                };

                let chain = if let Some(chain_cert) = self.chain_cert.clone() {
                    let pem = std::fs::read(chain_cert)?;
                    let chain = X509::stack_from_pem(&pem)?;

                    let mut stack = Stack::new()?;
                    for cert in chain {
                        stack.push(cert)?;
                    }
                    Some(stack)
                } else {
                    None
                };

                let key = if let Some(key_path) = self.tls_key.clone() {
                    let pem = std::fs::read(key_path)?;
                    Some(PKey::private_key_from_pem(&pem)?)
                } else {
                    None
                };

                tokio::spawn(
                    server
                        .clone()
                        .listen(ip, Duration::new(1, 0), tls_cert, chain, key),
                );
            }

            return Ok(ztauthority);
        }

        return Err(anyhow!(
            "No listening IPs for your interface; assign one in ZeroTier Central."
        ));
    }
}
