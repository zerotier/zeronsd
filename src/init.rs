use std::{collections::HashMap, path::PathBuf, str::FromStr, time::Duration};

use anyhow::anyhow;
use ipnetwork::IpNetwork;
use log::{info, warn};
use serde::{Deserialize, Serialize};

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
            ConfigFormat::JSON => serde_json::from_str(&s)?,
            ConfigFormat::YAML => serde_yaml::from_str(&s)?,
            ConfigFormat::TOML => toml::from_str(&s)?,
        })
    }

    pub fn parse(s: &str, network_id: String, format: ConfigFormat) -> Result<Self, anyhow::Error> {
        let mut l: Launcher = Self::parse_format(s, format)?;
        l.network_id = Some(network_id);
        Ok(l)
    }

    pub async fn start(&self) -> Result<ZTAuthority, anyhow::Error> {
        if self.network_id.is_none() {
            return Err(anyhow!("network ID is invalid; cannot continue"));
        }

        let domain_name = domain_or_default(self.domain.as_deref())?;
        let authtoken = authtoken_path(self.secret.as_deref());
        let config = central_config(central_token(self.token.as_deref())?);

        info!("Welcome to ZeroNS!");
        let ips = get_listen_ips(&authtoken, &self.network_id.clone().unwrap()).await?;

        // more or less the setup for the "main loop"
        if ips.len() > 0 {
            update_central_dns(
                domain_name.clone(),
                ips.iter()
                    .map(|i| parse_ip_from_cidr(i.clone()).to_string())
                    .collect(),
                config.clone(),
                self.network_id.clone().unwrap(),
            )
            .await?;

            let mut listen_ips = Vec::new();
            let mut ipmap = HashMap::new();
            let mut authority_map = HashMap::new();
            let authority = RecordAuthority::new(domain_name.clone().into()).await?;

            for cidr in ips.clone() {
                let listen_ip = parse_ip_from_cidr(cidr.clone());
                listen_ips.push(listen_ip.clone());
                let cidr = IpNetwork::from_str(&cidr.clone())?;
                if !ipmap.contains_key(&listen_ip) {
                    ipmap.insert(listen_ip, cidr.network());
                }

                if !authority_map.contains_key(&cidr) {
                    log::debug!("{}", cidr.to_ptr_soa_name()?);
                    let ptr_authority = RecordAuthority::new(cidr.to_ptr_soa_name()?).await?;
                    authority_map.insert(cidr, ptr_authority);
                }
            }

            let network = zerotier_central_api::apis::network_api::get_network_by_id(
                &config,
                &self.network_id.clone().unwrap(),
            )
            .await?;

            let v6assign = network.config.clone().unwrap().v6_assign_mode;
            if v6assign.is_some() {
                let v6assign = v6assign.unwrap().clone();

                if v6assign.var_6plane.unwrap_or(false) {
                    warn!("6PLANE PTR records are not yet supported");
                }

                if v6assign.rfc4193.unwrap_or(false) {
                    let cidr = network.clone().rfc4193().unwrap();
                    if !authority_map.contains_key(&cidr) {
                        let ptr_authority = RecordAuthority::new(cidr.to_ptr_soa_name()?).await?;
                        authority_map.insert(cidr, ptr_authority);
                    }
                }
            }

            let ztauthority = ZTAuthority {
                config,
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
                    Some(X509::stack_from_pem(&pem)?)
                } else {
                    None
                };

                let key = if let Some(key_path) = self.tls_key.clone() {
                    let pem = std::fs::read(key_path)?;
                    Some(PKey::private_key_from_pem(&pem)?)
                } else {
                    None
                };

                let certs = if chain.is_some() {
                    let mut stack = Stack::new()?;
                    for cert in chain.unwrap() {
                        stack.push(cert)?;
                    }
                    Some((tls_cert.unwrap(), Some(stack)))
                } else if tls_cert.is_some() {
                    Some((tls_cert.unwrap(), None))
                } else {
                    None
                };

                tokio::spawn(server.clone().listen(ip, Duration::new(1, 0), certs, key));
            }

            return Ok(ztauthority);
        }

        return Err(anyhow!(
            "No listening IPs for your interface; assign one in ZeroTier Central."
        ));
    }
}
