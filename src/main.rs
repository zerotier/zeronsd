use authority::ZTAuthority;
use server::Server;
use std::io::Write;
use std::str::FromStr;
use std::time::Duration;
use tokio::runtime::Runtime;
use trust_dns_server::client::rr::Name;
use zerotier_central_api::apis::configuration::Configuration;

extern crate clap;
use clap::clap_app;

use anyhow::anyhow;

mod authority;
mod hosts;
mod server;
mod supervise;

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

fn central_token(arg: Option<&str>) -> Option<String> {
    if arg.is_some() {
        return Some(
            std::fs::read_to_string(arg.unwrap())
                .expect("Could not load token file")
                .trim()
                .to_string(),
        );
    }

    if let Ok(token) = std::env::var("ZEROTIER_CENTRAL_TOKEN") {
        if token.len() > 0 {
            return Some(token);
        }
    }

    None
}

fn authtoken_path(arg: Option<&str>) -> String {
    if let Some(arg) = arg {
        return String::from(arg);
    } else {
        if cfg!(target_os = "linux") {
            String::from("/var/lib/zerotier-one/authtoken.secret")
        } else if cfg!(target_os = "windows") {
            String::from("C:/ProgramData/ZeroTier/One/authtoken.secret")
        } else if cfg!(target_os = "macos") {
            String::from("/Library/Application Support/ZeroTier/One/authtoken.secret")
        } else {
            panic!(
                "authtoken.secret not found; please provide the -s option to provide a custom path"
            )
        }
    }
}

fn domain_or_default(tld: Option<&str>) -> Result<Name, anyhow::Error> {
    if let Some(tld) = tld {
        if tld.len() > 0 {
            return Ok(Name::from_str(&format!("{}.", tld))?);
        } else {
            return Err(anyhow!("Domain name must not be empty if provided."));
        }
    };

    Ok(Name::from_str(crate::authority::DOMAIN_NAME)?)
}

async fn get_listen_ip(authtoken_path: &str, network_id: &str) -> Result<String, anyhow::Error> {
    let authtoken = std::fs::read_to_string(authtoken_path)?;
    let mut configuration = zerotier_one_api::apis::configuration::Configuration::default();
    let api_key = zerotier_one_api::apis::configuration::ApiKey {
        prefix: None,
        key: authtoken,
    };
    configuration.api_key = Some(api_key);

    let listen =
        zerotier_one_api::apis::network_api::get_network(&configuration, network_id).await?;
    if let Some(assigned) = listen.assigned_addresses {
        if let Some(ip) = assigned.first() {
            // for now, we'll use the first addr returned. Soon, we will want to listen on all IPs.
            return Ok(ip.clone());
        }
    }

    Err(anyhow!("No listen IPs available on this network"))
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

fn central_config(token: String) -> Configuration {
    let mut config = Configuration::default();
    config.bearer_access_token = Some(token);
    return config;
}

fn init_runtime() -> Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(4)
        .thread_name("zeronsd")
        .build()
        .expect("failed to initialize tokio")
}

fn parse_ip_from_cidr(ip_with_cidr: String) -> String {
    ip_with_cidr.splitn(2, "/").next().unwrap().to_string()
}

fn init_authority(
    runtime: &mut Runtime,
    token: String,
    network: String,
    domain_name: Name,
    hosts_file: Option<String>,
    ip_with_cidr: String,
    ip: String,
) -> Result<Server, anyhow::Error> {
    let config = central_config(token);

    let authority = ZTAuthority::new(
        domain_name.clone(),
        network.clone(),
        config.clone(),
        hosts_file,
        ip_with_cidr.clone(),
    )?;

    let owned = authority.to_owned();
    runtime.spawn(owned.find_members());

    let mut zt_network = runtime.block_on(
        zerotier_central_api::apis::network_api::get_network_by_id(&config, &network),
    )?;

    let mut domain_name = domain_name.clone();
    domain_name.set_fqdn(false);

    let dns = Some(Box::new(zerotier_central_api::models::NetworkConfigDns {
        domain: Some(domain_name.to_string()),
        servers: Some(Vec::from([String::from(ip.clone())])),
    }));

    if let Some(mut zt_network_config) = zt_network.config.to_owned() {
        zt_network_config.dns = dns;
        zt_network.config = Some(zt_network_config);
        runtime.block_on(zerotier_central_api::apis::network_api::update_network(
            &config, &network, zt_network,
        ))?;
    }

    Ok(crate::server::Server::new(
        authority.clone().catalog(runtime)?,
    ))
}

fn start(
    domain: Option<&str>,
    network: Option<&str>,
    hosts_file: Option<&str>,
    authtoken: Option<&str>,
    token: Option<&str>,
) -> Result<(), anyhow::Error> {
    let domain_name = domain_or_default(domain)?;
    let authtoken = authtoken_path(authtoken);
    let runtime = &mut init_runtime();

    if let Some(network) = network {
        let ip_with_cidr = runtime.block_on(get_listen_ip(&authtoken, network))?;
        let ip = parse_ip_from_cidr(ip_with_cidr.clone());

        println!("Welcome to ZeroNS!");
        println!("Your IP for this network: {}", ip);

        if let Some(token) = central_token(token) {
            let network = String::from(network);
            let hf = if let Some(hf) = hosts_file {
                Some(hf.to_string())
            } else {
                None
            };

            let server = init_authority(
                runtime,
                token,
                network,
                domain_name,
                hf,
                ip_with_cidr,
                ip.clone(),
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
        (version: env!("CARGO_PKG_VERSION"))
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
