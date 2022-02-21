use crate::{
    init::{ConfigFormat, Launcher},
    supervise::Properties,
};
use log::error;
use std::path::PathBuf;

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

#[derive(Args)]
pub struct StartArgs {
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
    pub wildcard: Option<bool>,

    /// Network ID to query
    pub network_id: String,

    /// Configuration file containing these arguments (overrides most CLI options)
    #[clap(short = 'c', long = "config", value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Configuration file format [yaml, json, toml]
    #[clap(long = "config-type", default_value = "yaml")]
    pub config_type: ConfigFormat,
}

impl Into<Launcher> for StartArgs {
    fn into(self) -> Launcher {
        if let Some(config) = self.config {
            let res = Launcher::new_from_config(config.to_str().unwrap(), self.config_type);
            match res {
                Ok(mut res) => {
                    res.network_id = self.network_id.clone();
                    res
                }
                Err(e) => {
                    log::error!("{}", e);
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
                network_id: self.network_id,
            }
        }
    }
}

#[derive(Args)]
pub struct UnsuperviseArgs {
    /// Network ID to remove
    pub network_id: String,
}

pub fn init() -> Result<(), anyhow::Error> {
    crate::utils::init_logger();

    let cli = Cli::parse();

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

fn start(args: StartArgs) -> Result<(), anyhow::Error> {
    let launcher: Launcher = args.into();
    launcher.start()
}

fn unsupervise(args: UnsuperviseArgs) -> Result<(), anyhow::Error> {
    Properties::from(args).uninstall_supervisor()
}

fn supervise(args: StartArgs) -> Result<(), anyhow::Error> {
    Properties::from(args).install_supervisor()
}
