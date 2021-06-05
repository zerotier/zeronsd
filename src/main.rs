use std::{io::Write, time::Duration};

use clap::clap_app;

use anyhow::anyhow;

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
        let ip_with_cidr = runtime.block_on(utils::get_listen_ip(&authtoken, network))?;
        let ip = utils::parse_ip_from_cidr(ip_with_cidr.clone());

        println!("Welcome to ZeroNS!");
        println!("Your IP for this network: {}", ip);

        if let Some(token) = utils::central_token(token) {
            let network = String::from(network);
            let hf = if let Some(hf) = hosts_file {
                Some(hf.to_string())
            } else {
                None
            };

            let server = utils::init_authority(
                runtime,
                token,
                network,
                domain_name,
                hf,
                ip_with_cidr,
                ip.clone(),
                Duration::new(30, 0),
            )?;

            runtime.block_on(server.listen(format!("{}:53", ip.clone()), Duration::new(0, 1000)))
        } else {
            Err(anyhow!("missing zerotier central token: set ZEROTIER_CENTRAL_TOKEN in environment, or pass a file containing it with -t"))
        }
    } else {
        return Err(anyhow!("no network ID"));
    }
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
            app.clone().write_long_help(&mut lock).unwrap();
            writeln!(lock)?;
            return Ok(());
        }
    }

    Ok(())
}
