use std::{
    collections::HashMap, net::SocketAddr, str::FromStr, sync::Arc, thread::sleep, time::Duration,
};

use anyhow::anyhow;
use clap::Parser;
use ipnetwork::IpNetwork;
use log::{error, info, warn};
use tokio::sync::RwLock;

use crate::{addresses::*, authority::*, cli::*, server::*, supervise::*, utils::*};

fn unsupervise(args: UnsuperviseArgs) -> Result<(), anyhow::Error> {
    Properties::from(args).uninstall_supervisor()
}

fn supervise(args: SuperviseArgs) -> Result<(), anyhow::Error> {
    Properties::from(args).install_supervisor()
}

fn start(args: StartArgs) -> Result<(), anyhow::Error> {
    let domain_name = domain_or_default(args.domain.as_deref())?;
    let authtoken = authtoken_path(args.secret.as_deref());
    let runtime = &mut init_runtime();
    let token = central_config(central_token(args.token.as_deref())?);

    info!("Welcome to ZeroNS!");
    let ips = runtime.block_on(get_listen_ips(&authtoken, &args.network_id))?;

    // more or less the setup for the "main loop"
    if ips.len() > 0 {
        update_central_dns(
            runtime,
            domain_name.clone(),
            ips.iter()
                .map(|i| parse_ip_from_cidr(i.clone()).to_string())
                .collect(),
            token.clone(),
            args.network_id.clone(),
        )?;

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

        let network = runtime.block_on(
            zerotier_central_api::apis::network_api::get_network_by_id(&token, &args.network_id),
        )?;

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
            args.network_id.clone(),
            token.clone(),
            args.hosts.clone(),
            authority_map.clone(),
            Duration::new(30, 0),
            authority.clone(),
        );

        if args.wildcard {
            ztauthority.wildcard_everything();
        }

        let arc_authority = Arc::new(RwLock::new(ztauthority));

        runtime.spawn(find_members(arc_authority.clone()));

        for ip in listen_ips {
            info!("Your IP for this network: {}", ip);

            let server = Server::new(arc_authority.to_owned());
            runtime.spawn(server.listen(SocketAddr::new(ip, 53), Duration::new(0, 1000)));
        }

        async fn wait() {
            loop {
                sleep(Duration::new(60, 0))
            }
        }

        return Ok(runtime.block_on(wait()));
    }

    return Err(anyhow!(
        "No listening IPs for your interface; assign one in ZeroTier Central."
    ));
}

pub fn init() -> Result<(), anyhow::Error> {
    init_logger();

    let cli = Cli::parse();

    let result = match cli.command {
        Command::Start(args) => start(args),
        Command::Supervise(args) => supervise(args),
        Command::Unsupervise(args) => unsupervise(args),
    };

    if result.is_err() {
        error!("{}", result.unwrap_err())
    }

    Ok(())
}
