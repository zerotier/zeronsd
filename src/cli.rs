use std::path::PathBuf;

use clap::{Args, Subcommand, Parser};

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
    Supervise(SuperviseArgs),

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
    pub wildcard: bool,

    /// Network ID to query
    pub network_id: String,
}

#[derive(Args)]
pub struct SuperviseArgs {
    /// TLD to use for hostnames
    #[clap(short, long)]
    pub domain: Option<String>,

    /// An additional list of hosts in /etc/hosts format
    #[clap(short = 'f', long = "file", value_name = "PATH")]
    pub hosts: Option<PathBuf>,

    /// Path to authtoken.secret (usually detected)
    #[clap(short, long, value_name = "PATH")]
    pub secret: Option<PathBuf>,

    /// Path to a file containing the ZeroTier Central token; this file must not be moved
    #[clap(short, long, value_name = "PATH")]
    pub token: Option<PathBuf>,

    /// Wildcard all names in Central to point at the respective member's IP address(es)
    #[clap(short, long)]
    pub wildcard: bool,

    /// Network ID to query
    pub network_id: String,
}

#[derive(Args)]
pub struct UnsuperviseArgs {
    /// Network ID to remove
    pub network_id: String,
}
