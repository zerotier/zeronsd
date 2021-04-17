use authority::ZTAuthority;
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
    return Ok(app.clone().write_long_help(&mut lock)?);
}

async fn start(
    domain: Option<&str>,
    network: Option<&str>,
    listen: Option<&str>,
) -> Result<(), anyhow::Error> {
    let domain_name = if let Some(tld) = domain {
        Name::from_str(&format!("{}.", tld))?
    } else {
        Name::from_str(crate::authority::DOMAIN_NAME)?
    };

    if let Some(network) = network {
        let authority = ZTAuthority::new(domain_name.clone(), 1, String::from(network))?;
        let owned = authority.to_owned();
        tokio::spawn(owned.find_members());

        if let Some(ip) = listen {
            let server = crate::server::Server::new(authority.clone().catalog());
            server
                .listen(&format!("{}:53", ip), Duration::new(0, 1000))
                .await
        } else {
            return Err(anyhow!("no listen IP"));
        }
    } else {
        return Err(anyhow!("no network ID"));
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
    );

    let matches = app.clone().get_matches();

    let (cmd, args) = matches.subcommand();
    let args = match args {
        Some(args) => args,
        None => return write_help(app),
    };

    match cmd {
        "start" => {
            start(
                args.value_of("domain"),
                args.value_of("NETWORK_ID"),
                args.value_of("LISTEN_IP"),
            )
            .await?
        }
        _ => {
            let stderr = std::io::stderr();
            let mut lock = stderr.lock();
            app.clone().write_long_help(&mut lock).unwrap();
            return Ok(());
        }
    }

    Ok(())
}
