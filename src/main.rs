use std::{
    collections::HashMap, io::Write, net::SocketAddr, str::FromStr, sync::Arc, thread::sleep,
    time::Duration,
};

use clap::clap_app;

use anyhow::anyhow;
use ipnetwork::IpNetwork;
use log::{error, info, warn};
use tokio::sync::RwLock;

use crate::{
    addresses::Calculator,
    authority::{find_members, init_trust_dns_authority, new_ptr_authority, ZTAuthority},
    utils::{central_config, central_token, update_central_dns},
};

mod addresses;
mod authority;
mod hosts;
mod server;
mod supervise;
mod utils;

#[cfg(all(feature = "integration-tests", test))]
mod integration_tests;
#[cfg(test)]
mod tests;

fn write_help(app: clap::App) -> Result<(), anyhow::Error> {
    let stderr = std::io::stderr();
    let mut lock = stderr.lock();
    app.clone().write_long_help(&mut lock)?;
    writeln!(lock)?;
    return Ok(());
}

fn unsupervise(args: &clap::ArgMatches<'_>) -> Result<(), anyhow::Error> {
    supervise::Properties::from(args).uninstall_supervisor()
}

fn supervise(args: &clap::ArgMatches<'_>) -> Result<(), anyhow::Error> {
    supervise::Properties::from(args).install_supervisor()
}

fn start(args: &clap::ArgMatches<'_>) -> Result<(), anyhow::Error> {
    let domain = args.value_of("domain");
    let authtoken = args.value_of("secret_file");
    let network_id = args.value_of("NETWORK_ID");
    let hosts_file = args.value_of("file");
    let token = args.value_of("token_file");
    let wildcard_names = args.is_present("wildcard");

    let domain_name = utils::domain_or_default(domain)?;
    let authtoken = utils::authtoken_path(authtoken);
    let runtime = &mut utils::init_runtime();

    if let Some(network_id) = network_id {
        let token = central_config(central_token(token)?);
        let network_id = String::from(network_id);

        let hosts_file = if let Some(hf) = hosts_file {
            Some(hf.to_string())
        } else {
            None
        };

        info!("Welcome to ZeroNS!");
        let ips = runtime.block_on(utils::get_listen_ips(&authtoken, &network_id))?;

        if ips.len() > 0 {
            update_central_dns(
                runtime,
                domain_name.clone(),
                ips.clone(),
                token.clone(),
                network_id.clone(),
            )?;

            let mut listen_ips = Vec::new();
            let mut ipmap = HashMap::new();
            let mut authority_map = HashMap::new();
            let authority = init_trust_dns_authority(domain_name.clone());

            for cidr in ips.clone() {
                let listen_ip = utils::parse_ip_from_cidr(cidr.clone());
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
                zerotier_central_api::apis::network_api::get_network_by_id(&token, &network_id),
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

            let mut ztauthority = ZTAuthority::new(
                domain_name.clone(),
                network_id.clone(),
                token.clone(),
                hosts_file.clone(),
                authority_map.clone(),
                Duration::new(30, 0),
                authority.clone(),
            );

            if wildcard_names {
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

            return Ok(runtime.block_on(wait()));
        }

        return Err(anyhow!(
            "No listening IPs for your interface; assign one in ZeroTier Central."
        ));
    }

    return Err(anyhow!("no network ID"));
}

fn main() -> Result<(), anyhow::Error> {
    let app = clap::clap_app!(zeronsd =>
        (author: "Erik Hollensbe <github@hollensbe.org>")
        (about: "zerotier central nameserver")
        (version: utils::VERSION_STRING)
        (@subcommand start =>
            (about: "Start the nameserver")
            (@arg domain: -d --domain +takes_value "TLD to use for hostnames")
            (@arg file: -f --file +takes_value "An additional lists of hosts in /etc/hosts format")
            (@arg secret_file: -s --secret +takes_value "Path to authtoken.secret (usually detected)")
            (@arg token_file: -t --token +takes_value "Path to a file containing the ZeroTier Central token")
            (@arg wildcard: -w --wildcard "Wildcard all names in Central to point at the respective member's IP address(es)")
            (@arg verbose: -v +multiple "Verbose logging (repeat -v for more verbosity)")
            (@arg NETWORK_ID: +required "Network ID to query")
        )
        (@subcommand supervise =>
            (about: "Configure supervision of the nameserver for a single network")
            (@arg domain: -d --domain +takes_value "TLD to use for hostnames")
            (@arg file: -f --file +takes_value "An additional lists of hosts in /etc/hosts format")
            (@arg secret_file: -s --secret +takes_value "Path to authtoken.secret (usually detected)")
            (@arg token_file: -t --token +takes_value +required "Path to a file containing the ZeroTier Central token; this file must not be moved")
            (@arg wildcard: -w --wildcard "Wildcard all names in Central to point at the respective member's IP address(es)")
            (@arg NETWORK_ID: +required "Network ID to query")
        )
        (@subcommand unsupervise =>
            (about: "Remove supervision of the nameserver for a network")
            (@arg NETWORK_ID: +required "Network ID to remove")
        )
    );

    let matches = app.clone().get_matches().clone();

    let (cmd, args) = matches.subcommand();
    let args = match args {
        Some(args) => args,
        None => return write_help(app),
    };

    stderrlog::new()
        .module(String::from("zeronsd"))
        .verbosity((args.occurrences_of("verbose") + 2) as usize)
        .timestamp(stderrlog::Timestamp::Off)
        .init()
        .unwrap();

    let result = match cmd {
        "start" => start(args),
        "supervise" => supervise(args),
        "unsupervise" => unsupervise(args),
        _ => {
            let stderr = std::io::stderr();
            let mut lock = stderr.lock();
            app.clone()
                .write_long_help(&mut lock)
                .expect("Could not write help to stdio: Welp.");
            writeln!(lock)?;
            return Ok(());
        }
    };

    if result.is_err() {
        error!("{}", result.unwrap_err())
    }

    Ok(())
}
