use authority::ZTAuthority;
use central::apis::configuration::Configuration;
use std::io::Write;
use std::str::FromStr;
use std::time::Duration;
use trust_dns_server::client::rr::Name;

extern crate clap;
use clap::clap_app;

use anyhow::anyhow;

mod authority;
mod server;

fn write_help(app: clap::App) -> Result<(), anyhow::Error> {
    let stderr = std::io::stderr();
    let mut lock = stderr.lock();
    app.clone().write_long_help(&mut lock)?;
    writeln!(lock)?;
    return Ok(());
}

fn start(
    domain: Option<&str>,
    network: Option<&str>,
    listen: Option<&str>,
) -> Result<(), anyhow::Error> {
    let domain_name = if let Some(tld) = domain {
        Name::from_str(&format!("{}.", tld))?
    } else {
        Name::from_str(crate::authority::DOMAIN_NAME)?
    };

    let mut runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .thread_name("zeronsd")
        .build()
        .expect("failed to initialize tokio");

    if let Some(network) = network {
        if let Ok(token) = std::env::var("ZEROTIER_CENTRAL_TOKEN") {
            let network = String::from(network);
            let mut config = Configuration::default();
            config.bearer_access_token = Some(token.clone());

            let authority =
                ZTAuthority::new(domain_name.clone(), 1, network.clone(), config.clone())?;

            let owned = authority.to_owned();
            runtime.spawn(owned.find_members());

            if let Some(ip) = listen {
                let mut zt_network = runtime.block_on(
                    central::apis::network_api::get_network_by_id(&config, &network),
                )?;

                let mut domain_name = domain_name.clone();
                domain_name.set_fqdn(false);

                let dns = Some(Box::new(central::models::NetworkConfigDns {
                    domain: Some(domain_name.to_string()),
                    servers: Some(Vec::from([String::from(ip)])),
                }));

                if let Some(mut zt_network_config) = zt_network.config.to_owned() {
                    zt_network_config.dns = dns;
                    zt_network.config = Some(zt_network_config);
                    runtime.block_on(central::apis::network_api::update_network(
                        &config, &network, zt_network,
                    ))?;
                }

                let server = crate::server::Server::new(
                    authority.clone().catalog(&mut runtime)?,
                    config,
                    network,
                );

                runtime.block_on(server.listen(&format!("{}:53", ip), Duration::new(0, 1000)))
            } else {
                return Err(anyhow!("no listen IP"));
            }
        } else {
            Err(anyhow!("missing zerotier central token"))
        }
    } else {
        return Err(anyhow!("no network ID"));
    }
}

fn main() -> Result<(), anyhow::Error> {
    let app = clap::clap_app!(zeronsd =>
        (author: "Erik Hollensbe <github@hollensbe.org>")
        (about: "zerotier central nameserver")
        (version: "0.1.0")
        (@subcommand start =>
            (about: "Start the nameserver")
            (@arg domain: -d --domain +takes_value "TLD to use for hostnames")
            (@arg NETWORK_ID: +required "Network ID to query")
            (@arg LISTEN_IP: +required "IP address to listen on")
        )
    );

    let matches = app.clone().get_matches();

    let (cmd, args) = matches.subcommand();
    let args = match args {
        Some(args) => args,
        None => return write_help(app),
    };

    match cmd {
        "start" => start(
            args.value_of("domain"),
            args.value_of("NETWORK_ID"),
            args.value_of("LISTEN_IP"),
        )?,
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
