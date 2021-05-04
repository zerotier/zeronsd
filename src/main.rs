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

fn authtoken_path() -> Option<&'static str> {
    if cfg!(target_os = "linux") {
        Some("/var/lib/zerotier-one/authtoken.secret")
    } else if cfg!(target_os = "windows") {
        Some("/ProgramData/ZeroTier/One/authtoken.secret")
    } else if cfg!(target_os = "macos") {
        Some("/Library/Application Support/ZeroTier/One/authtoken.secret")
    } else {
        None
    }
}

async fn get_listen_ip(authtoken_path: &str, network_id: &str) -> Result<String, anyhow::Error> {
    let authtoken = std::fs::read_to_string(authtoken_path)?;
    let mut configuration = service::apis::configuration::Configuration::default();
    let api_key = service::apis::configuration::ApiKey {
        prefix: None,
        key: authtoken,
    };
    configuration.api_key = Some(api_key);

    let listen = service::apis::network_api::get_network(&configuration, network_id).await?;
    if let Some(assigned) = listen.assigned_addresses {
        if let Some(ip) = assigned.first() {
            // for now, we'll use the first addr returned. Soon, we will want to listen on all IPs.
            return Ok(ip.clone());
        }
    }

    Err(anyhow!("No listen IPs available on this network"))
}

fn start(
    domain: Option<&str>,
    network: Option<&str>,
    hosts_file: Option<&str>,
    authtoken: Option<&str>,
) -> Result<(), anyhow::Error> {
    let domain_name = if let Some(tld) = domain {
        Name::from_str(&format!("{}.", tld))?
    } else {
        Name::from_str(crate::authority::DOMAIN_NAME)?
    };

    let authtoken = match authtoken {
        Some(p) => p,
        None => authtoken_path().expect(
            "authtoken.secret not found; please provide the -s option to provide a custom path",
        ),
    };

    let mut runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(4)
        .thread_name("zeronsd")
        .build()
        .expect("failed to initialize tokio");

    if let Some(network) = network {
        let ip_with_cidr = runtime.block_on(get_listen_ip(authtoken, network))?;
        let ip = ip_with_cidr.splitn(2, "/").next().unwrap();

        println!("Welcome to ZeroNS!");
        println!("Your IP for this network: {}", ip);

        if let Ok(token) = std::env::var("ZEROTIER_CENTRAL_TOKEN") {
            let network = String::from(network);
            let mut config = Configuration::default();
            config.bearer_access_token = Some(token.clone());

            let hf = if let Some(hf) = hosts_file {
                Some(hf.to_string())
            } else {
                None
            };

            let authority =
                ZTAuthority::new(domain_name.clone(), 1, network.clone(), config.clone(), hf)?;

            let owned = authority.to_owned();
            runtime.spawn(owned.find_members());

            let mut zt_network = runtime.block_on(
                central::apis::network_api::get_network_by_id(&config, &network),
            )?;

            let mut domain_name = domain_name.clone();
            domain_name.set_fqdn(false);

            let dns = Some(Box::new(central::models::NetworkConfigDns {
                domain: Some(domain_name.to_string()),
                servers: Some(Vec::from([String::from(ip.clone())])),
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

            runtime.block_on(server.listen(&format!("{}:53", ip.clone()), Duration::new(0, 1000)))
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
            (@arg file: -f --file +takes_value "An additional lists of hosts in /etc/hosts format")
            (@arg secret_file: -s --secret +takes_value "Path to authtoken.secret (usually detected)")
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
        "start" => start(
            args.value_of("domain"),
            args.value_of("NETWORK_ID"),
            args.value_of("file"),
            args.value_of("secret_file"),
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
