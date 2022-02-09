use std::{
    collections::{hash_map::Entry, HashMap},
    net::SocketAddr,
    str::FromStr,
    sync::Arc,
    thread::sleep,
    time::Duration,
};

use anyhow::ensure;
use clap::Parser;
use ipnetwork::IpNetwork;
use log::{error, info, warn};
use tokio::sync::RwLock;

use crate::{
    addresses::Calculator,
    authority::{find_members, init_trust_dns_authority, new_ptr_authority, ZTAuthority},
    cli::{Cli, Command, StartArgs, SuperviseArgs, UnsuperviseArgs},
    utils::{central_config, central_token, update_central_dns},
};

mod addresses;
mod authority;
mod cli;
mod hosts;
mod server;
mod supervise;
mod utils;

// integration tests are setup a little weird; basically `cargo test --feature integration-tests`
#[cfg(all(feature = "integration-tests", test))]
mod integration_tests;
#[cfg(test)]
mod tests;

fn unsupervise(args: UnsuperviseArgs) -> Result<(), anyhow::Error> {
    supervise::Properties::from(args).uninstall_supervisor()
}

fn supervise(args: SuperviseArgs) -> Result<(), anyhow::Error> {
    supervise::Properties::from(args).install_supervisor()
}

fn start(args: StartArgs) -> Result<(), anyhow::Error> {
    let domain_name = utils::domain_or_default(args.domain.as_deref())?;
    let authtoken = utils::authtoken_path(args.secret.as_deref());
    let runtime = &mut utils::init_runtime();
    let token = central_config(central_token(args.token.as_deref())?);

    info!("Welcome to ZeroNS!");
    let ips = runtime.block_on(utils::get_listen_ips(authtoken, &args.network_id))?;

    ensure!(
        !ips.is_empty(),
        "No listening IPs for your interface; assign one in ZeroTier Central."
    );

    // more or less the setup for the "main loop"
    update_central_dns(
        runtime,
        domain_name.clone(),
        ips.iter()
            .map(|i| utils::parse_ip_from_cidr(i.clone()).to_string())
            .collect(),
        token.clone(),
        args.network_id.clone(),
    )?;

    let mut listen_ips = Vec::new();
    let mut ipmap = HashMap::new();
    let mut authority_map = HashMap::new();
    let authority = init_trust_dns_authority(domain_name.clone());

    for cidr in ips.clone() {
        let listen_ip = utils::parse_ip_from_cidr(cidr.clone());
        listen_ips.push(listen_ip);
        let cidr = IpNetwork::from_str(&cidr.clone())?;
        ipmap.entry(listen_ip).or_insert_with(|| cidr.network());

        if let Entry::Vacant(entry) = authority_map.entry(cidr) {
            let ptr_authority = new_ptr_authority(cidr)?;
            entry.insert(ptr_authority);
        }
    }

    let network = runtime.block_on(zerotier_central_api::apis::network_api::get_network_by_id(
        &token,
        &args.network_id,
    ))?;

    let v6assign = network.config.clone().unwrap().v6_assign_mode;
    if let Some(v6assign) = v6assign {
        if v6assign.var_6plane.unwrap_or(false) {
            warn!("6PLANE PTR records are not yet supported");
        }

        if v6assign.rfc4193.unwrap_or(false) {
            let cidr = network.clone().rfc4193().unwrap();
            if let Entry::Vacant(entry) = authority_map.entry(cidr) {
                let ptr_authority = new_ptr_authority(cidr)?;
                entry.insert(ptr_authority);
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

        let server = crate::server::Server::new(arc_authority.to_owned());
        runtime.spawn(server.listen(SocketAddr::new(ip, 53), Duration::new(0, 1000)));
    }

    async fn wait() {
        loop {
            sleep(Duration::new(60, 0))
        }
    }

    runtime.block_on(wait());
    Ok(())
}

fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();

    stderrlog::new()
        .module(String::from("zeronsd"))
        .verbosity(cli.verbose + 2)
        .timestamp(stderrlog::Timestamp::Off)
        .init()
        .unwrap();

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
