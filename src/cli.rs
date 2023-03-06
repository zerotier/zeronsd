use crate::{
    init::{ConfigFormat, Launcher},
    supervise::Properties,
    utils::ZEROTIER_LOCAL_URL,
};
use std::{path::PathBuf, time::Duration};

use clap::{Args, Parser, Subcommand};

/// zerotier central nameserver
#[derive(Parser)]
#[clap(version, author = "Erik Hollensbe <github@hollensbe.org>")]
pub struct Cli {
    /// Verbose logging (repeat -v for more verbosity)
    #[clap(short, global = true, parse(from_occurrences))]
    pub verbose: usize,

    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Start the nameserver
    Start(StartArgs),

    /// Configure supervision of the nameserver for a single network
    Supervise(StartArgs),

    /// Remove supervision of the nameserver for a network
    Unsupervise(UnsuperviseArgs),
}

#[derive(Args, Clone)]
pub struct StartArgs {
    /// Network ID to query
    pub network_id: String,

    /// TLD to use for hostnames
    #[clap(short, long)]
    pub domain: Option<String>,

    /// An additional list of hosts in /etc/hosts format
    #[clap(short = 'f', long = "file", value_name = "PATH")]
    pub hosts: Option<PathBuf>,

    /// Path to authtoken.secret (usually detected)
    #[clap(short, long, value_name = "PATH")]
    pub secret: Option<PathBuf>,

    /// Path to a file containing the ZeroTier Central token
    #[clap(short, long, value_name = "PATH")]
    pub token: Option<PathBuf>,

    /// Wildcard all names in Central to point at the respective member's IP address(es)
    #[clap(short, long)]
    pub wildcard: bool,

    /// Configuration file containing these arguments (overrides most CLI options)
    #[clap(short = 'c', long = "config", value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Configuration file format [yaml, json, toml]
    #[clap(long = "config-type", default_value = "yaml")]
    pub config_type: ConfigFormat,

    #[clap(long = "tls-cert", value_name = "PATH")]
    pub tls_cert: Option<PathBuf>,

    #[clap(long = "chain-cert", value_name = "PATH")]
    pub chain_cert: Option<PathBuf>,

    #[clap(long = "tls-key", value_name = "PATH")]
    pub tls_key: Option<PathBuf>,

    /// Provide a different URL for contacting the local zerotier-one service. Default:
    #[clap(long = "local-url", value_name = "LOCAL_URL", default_value = ZEROTIER_LOCAL_URL)]
    pub local_url: String,

    /// Log Level to print [off, trace, debug, error, warn, info]
    #[clap(short = 'l', long = "log-level", value_name = "LEVEL")]
    pub log_level: Option<crate::log::LevelFilter>,
}

impl Into<Launcher> for StartArgs {
    fn into(self) -> Launcher {
        if let Some(config) = self.config {
            let res = Launcher::new_from_config(config.to_str().unwrap(), self.config_type);
            match res {
                Ok(mut res) => {
                    res.network_id = Some(self.network_id.clone());
                    res
                }
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            }
        } else {
            Launcher {
                domain: self.domain,
                hosts: self.hosts,
                secret: self.secret,
                token: self.token,
                wildcard: self.wildcard,
                chain_cert: self.chain_cert,
                tls_cert: self.tls_cert,
                tls_key: self.tls_key,
                log_level: self.log_level,
                network_id: Some(self.network_id),
                local_url: self.local_url,
            }
        }
    }
}

#[derive(Args)]
pub struct UnsuperviseArgs {
    /// Network ID to remove
    pub network_id: String,
}

pub async fn init() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Start(args) => {
            start(args).await?;

            loop {
                tokio::time::sleep(Duration::MAX).await
            }
        }
        Command::Supervise(args) => supervise(args),
        Command::Unsupervise(args) => unsupervise(args),
    };

    if result.is_err() {
        eprintln!("{}", result.unwrap_err())
    }

    Ok(())
}

async fn start(args: StartArgs) -> Result<(), anyhow::Error> {
    let launcher: Launcher = args.into();

    launcher.start().await?;
    Ok(())
}

fn unsupervise(args: UnsuperviseArgs) -> Result<(), anyhow::Error> {
    crate::utils::init_logger(Some(tracing::Level::INFO));
    Properties::from(args).uninstall_supervisor()
}

fn supervise(args: StartArgs) -> Result<(), anyhow::Error> {
    crate::utils::init_logger(Some(tracing::Level::INFO));
    Properties::from(args).install_supervisor()
}
