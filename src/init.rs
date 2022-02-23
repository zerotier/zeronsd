use std::{
    collections::HashMap, net::SocketAddr, path::PathBuf, str::FromStr, sync::Arc, time::Duration,
};

use anyhow::anyhow;
use ipnetwork::IpNetwork;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::{addresses::*, authority::*, server::*, utils::*};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Launcher {
    pub(crate) domain: Option<String>,
    pub(crate) hosts: Option<PathBuf>,
    pub(crate) secret: Option<PathBuf>,
    pub(crate) token: Option<PathBuf>,
    pub(crate) wildcard: bool,
    #[serde(skip_deserializing)]
    pub(crate) network_id: String,
}

#[derive(Debug, Clone)]
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

type ArcAuthority = Arc<RwLock<ZTAuthority>>;

impl Launcher {
    pub fn new_from_config(filename: &str, format: ConfigFormat) -> Result<Self, anyhow::Error> {
        let res = std::fs::read_to_string(filename)?;
        Ok(match format {
            ConfigFormat::JSON => serde_json::from_str(&res)?,
            ConfigFormat::YAML => serde_yaml::from_str(&res)?,
            ConfigFormat::TOML => toml::from_str(&res)?,
        })
    }

    pub async fn start(&self) -> Result<ArcAuthority, anyhow::Error> {
        let domain_name = domain_or_default(self.domain.as_deref())?;
        let authtoken = authtoken_path(self.secret.as_deref());
        let token = central_config(central_token(self.token.as_deref())?);

        info!("Welcome to ZeroNS!");
        let ips = get_listen_ips(&authtoken, &self.network_id).await?;

        // more or less the setup for the "main loop"
        if ips.len() > 0 {
            update_central_dns(
                domain_name.clone(),
                ips.iter()
                    .map(|i| parse_ip_from_cidr(i.clone()).to_string())
                    .collect(),
                token.clone(),
                self.network_id.clone(),
            )
            .await?;

            let mut listen_ips = Vec::new();
            let mut ipmap = HashMap::new();
            let mut authority_map = HashMap::new();
            let authority = init_trust_dns_authority(domain_name.clone());

            for cidr in ips.clone() {
                let listen_ip = parse_ip_from_cidr(cidr.clone());
                listen_ips.push(listen_ip.clone());
                let cidr = IpNetwork::from_str(&cidr.clone())?;
                if !ipmap.contains_key(&listen_ip) {
                    ipmap.insert(listen_ip, cidr.network());
                }

                if !authority_map.contains_key(&cidr) {
                    let ptr_authority = new_ptr_authority(cidr)?;
                    authority_map.insert(cidr, ptr_authority);
                }
            }

            let network = zerotier_central_api::apis::network_api::get_network_by_id(
                &token,
                &self.network_id,
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
                        let ptr_authority = new_ptr_authority(cidr)?;
                        authority_map.insert(cidr, ptr_authority);
                    }
                }
            }

            // ZTAuthority more or less is the mainloop. Setup continues below.
            let mut ztauthority = ZTAuthority::new(
                domain_name.clone(),
                self.network_id.clone(),
                token.clone(),
                self.hosts.clone(),
                authority_map.clone(),
                Duration::new(30, 0),
                authority.clone(),
            );

            if self.wildcard {
                ztauthority.wildcard_everything();
            }

            let arc_authority = Arc::new(RwLock::new(ztauthority));

            tokio::spawn(find_members(arc_authority.clone()));

            for ip in listen_ips {
                info!("Your IP for this network: {}", ip);

                let server = Server::new(arc_authority.to_owned());
                tokio::spawn(server.listen(SocketAddr::new(ip, 53), Duration::new(0, 1000)));
            }

            return Ok(arc_authority);
        }

        return Err(anyhow!(
            "No listening IPs for your interface; assign one in ZeroTier Central."
        ));
    }
}
