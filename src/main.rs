use lib::configure_authority;
use openapi::{apis::configuration::Configuration, models::Member};
use std::{str::FromStr, time::Duration};
use trust_dns_server::{authority::Catalog, client::rr::Name};

extern crate clap;
use clap::clap_app;

use anyhow::anyhow;

mod lib;
mod server;

const DOMAIN_NAME: &str = "domain.";

fn write_help(app: clap::App) -> Result<(), anyhow::Error> {
    let stderr = std::io::stderr();
    let mut lock = stderr.lock();
    return Ok(app.clone().write_long_help(&mut lock)?);
}

async fn start(
    app: clap::App<'static, 'static>,
    args: &clap::ArgMatches<'static>,
) -> Result<(), anyhow::Error> {
    let domain_name = if let Some(tld) = args.value_of("domain") {
        Name::from_str(&format!("{}.", tld))?
    } else {
        Name::from_str(DOMAIN_NAME)?
    };

    let mut authority = crate::lib::new_authority(&domain_name.to_string())?;

    match get_members(args).await {
        Ok(members) => {
            configure_authority(&mut authority, domain_name.clone(), 1, members)?;

            let mut catalog = Catalog::default();
            catalog.upsert(
                domain_name.clone().into(),
                Box::new(std::sync::Arc::new(std::sync::RwLock::new(authority))),
            );

            if let Some(ip) = args.value_of("LISTEN_IP") {
                let server = crate::server::Server::new(catalog);
                server
                    .listen(&format!("{}:53", ip), Duration::new(0, 1000))
                    .await
            } else {
                write_help(app)
            }
        }
        Err(e) => {
            eprintln!("{}", e);
            write_help(app)
        }
    }
}

async fn get_members(args: &clap::ArgMatches<'static>) -> Result<Vec<Member>, anyhow::Error> {
    let network = args.value_of("NETWORK_ID").unwrap();
    let mut config = Configuration::default();
    if let Ok(token) = std::env::var("ZEROTIER_CENTRAL_TOKEN") {
        config.bearer_access_token = Some(token);
        let list =
            openapi::apis::network_member_api::get_network_member_list(&config, network).await?;
        Ok(list)
    } else {
        Err(anyhow!("missing zerotier central token"))
    }
}

async fn dump(app: clap::App<'static, 'static>, args: &clap::ArgMatches<'static>) {
    match get_members(args).await {
        Ok(members) => {
            for member in members {
                println!(
                    "{} {}",
                    member.node_id.unwrap(),
                    member
                        .config
                        .clone()
                        .unwrap()
                        .ip_assignments
                        .unwrap()
                        .join(" "),
                );

                if let Some(name) = member.name {
                    println!(
                        "{} {}",
                        name,
                        member
                            .config
                            .clone()
                            .unwrap()
                            .ip_assignments
                            .unwrap()
                            .join(" "),
                    );
                }
            }
        }
        Err(e) => {
            eprintln!("{}", e);
            write_help(app).unwrap();
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let app = clap::clap_app!(hostsns =>
        (author: "Erik Hollensbe <github@hollensbe.org>")
        (about: "zerotier central nameserver")
        (version: "0.1.0")
        (@subcommand start =>
            (about: "Start the nameserver")
            (@arg domain: -d --domain +takes_value "TLD to use for hostnames")
            (@arg NETWORK_ID: +required "Network ID to query")
            (@arg LISTEN_IP: +required "IP address to listen on")
        )
        (@subcommand dump =>
            (about: "Dump a hosts file of the network")
            (@arg NETWORK_ID: +required "Network ID to query")
        )
    );

    let matches = app.clone().get_matches();

    let (cmd, args) = matches.subcommand();
    let args = match args {
        Some(args) => args,
        None => return write_help(app),
    };

    match cmd {
        "start" => start(app, args).await?,
        "dump" => dump(app, args).await,
        _ => {
            let stderr = std::io::stderr();
            let mut lock = stderr.lock();
            app.clone().write_long_help(&mut lock).unwrap();
            return Ok(());
        }
    }

    Ok(())
}
