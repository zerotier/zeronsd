use std::{collections::HashMap, io::Write, str::FromStr, thread::sleep, time::Duration};

use clap::clap_app;

use anyhow::anyhow;
use ipnetwork::IpNetwork;

use crate::{authority::new_ptr_authority, utils::update_central_dns};

mod authority;
mod hosts;
mod server;
mod supervise;
mod utils;

#[cfg(test)]
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

fn unsupervise(network: Option<&str>) -> Result<(), anyhow::Error> {
    supervise::Properties::new(None, network, None, None, None)?.uninstall_supervisor()
}

fn supervise(
    domain: Option<&str>,
    network: Option<&str>,
    hosts_file: Option<&str>,
    authtoken: Option<&str>,
    token: Option<&str>,
) -> Result<(), anyhow::Error> {
    supervise::Properties::new(domain, network, hosts_file, authtoken, token)?.install_supervisor()
}

fn start(
    domain: Option<&str>,
    network: Option<&str>,
    hosts_file: Option<&str>,
    authtoken: Option<&str>,
    token: Option<&str>,
) -> Result<(), anyhow::Error> {
    let domain_name = utils::domain_or_default(domain)?;
    let authtoken = utils::authtoken_path(authtoken);
    let runtime = &mut utils::init_runtime();

    if let Some(network) = network {
        let token = utils::central_token(token);
        let network = String::from(network);
        let hf = if let Some(hf) = hosts_file {
            Some(hf.to_string())
        } else {
            None
        };

        if token.is_none() {
            return Err(anyhow!("missing zerotier central token: set ZEROTIER_CENTRAL_TOKEN in environment, or pass a file containing it with -t"));
        }

        let token = token.unwrap();

        println!("Welcome to ZeroNS!");

        let ips = runtime.block_on(utils::get_listen_ips(&authtoken, &network))?;

        if ips.len() > 0 {
            update_central_dns(
                runtime,
                domain_name.clone(),
                ips.first().unwrap().to_string(),
                token.clone(),
                network.clone(),
            )?;

            let mut listen_ips = Vec::new();
            let mut ipmap = HashMap::new();
            let mut ptrmap = HashMap::new();

            for cidr in ips.clone() {
                let listen_ip = utils::parse_ip_from_cidr(cidr.clone());
                listen_ips.push(listen_ip.clone());
                let cidr = IpNetwork::from_str(&cidr.clone())?;
                if !ipmap.contains_key(&listen_ip) {
                    ipmap.insert(listen_ip, cidr);
                    ptrmap.insert(cidr, new_ptr_authority(cidr)?);
                }
            }

            for ip in listen_ips {
                println!("Your IP for this network: {}", ip);
                let cidr = ipmap
                    .get(&ip)
                    .expect("Could not locate underlying network subnet");
                let ptr_authority = ptrmap
                    .get(cidr)
                    .expect("Could not locate PTR authority for subnet");

                let server = utils::init_authority(
                    runtime,
                    ptr_authority.clone(),
                    token.clone(),
                    network.clone(),
                    domain_name.clone(),
                    hf.clone(),
                    Duration::new(30, 0),
                )?;

                runtime.spawn(server.listen(format!("{}:53", ip.clone()), Duration::new(0, 1000)));
            }

            async fn wait() {
                loop {
                    sleep(Duration::new(60, 0))
                }
            }

            return Ok(runtime.block_on(wait()));
        }
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
            (@arg NETWORK_ID: +required "Network ID to query")
        )
        (@subcommand supervise =>
            (about: "Configure supervision of the nameserver for a single network")
            (@arg domain: -d --domain +takes_value "TLD to use for hostnames")
            (@arg file: -f --file +takes_value "An additional lists of hosts in /etc/hosts format")
            (@arg secret_file: -s --secret +takes_value "Path to authtoken.secret (usually detected)")
            (@arg token_file: -t --token +takes_value +required "Path to a file containing the ZeroTier Central token; this file must not be moved")
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

    match cmd {
        "start" => start(
            args.value_of("domain"),
            args.value_of("NETWORK_ID"),
            args.value_of("file"),
            args.value_of("secret_file"),
            args.value_of("token_file"),
        )?,
        "supervise" => supervise(
            args.value_of("domain"),
            args.value_of("NETWORK_ID"),
            args.value_of("file"),
            args.value_of("secret_file"),
            args.value_of("token_file"),
        )?,
        "unsupervise" => unsupervise(args.value_of("NETWORK_ID"))?,
        _ => {
            let stderr = std::io::stderr();
            let mut lock = stderr.lock();
            app.clone()
                .write_long_help(&mut lock)
                .expect("Could not write help to stdio: Welp.");
            writeln!(lock)?;
            return Ok(());
        }
    }

    Ok(())
}
