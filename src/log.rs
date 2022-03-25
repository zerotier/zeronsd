use std::str::FromStr;

use anyhow::anyhow;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LevelFilter {
    #[serde(rename(deserialize = "off"))]
    Off,
    #[serde(rename(deserialize = "error"))]
    Error,
    #[serde(rename(deserialize = "warn"))]
    Warn,
    #[serde(rename(deserialize = "info"))]
    Info,
    #[serde(rename(deserialize = "trace"))]
    Trace,
    #[serde(rename(deserialize = "debug"))]
    Debug,
}

impl LevelFilter {
    pub fn to_log(&self) -> log::LevelFilter {
        match self {
            LevelFilter::Off => log::LevelFilter::Off,
            LevelFilter::Error => log::LevelFilter::Error,
            LevelFilter::Warn => log::LevelFilter::Warn,
            LevelFilter::Info => log::LevelFilter::Info,
            LevelFilter::Trace => log::LevelFilter::Trace,
            LevelFilter::Debug => log::LevelFilter::Debug,
        }
    }
}

impl ToString for LevelFilter {
    fn to_string(&self) -> String {
        match self {
            LevelFilter::Off => "off",
            LevelFilter::Error => "error",
            LevelFilter::Warn => "warn",
            LevelFilter::Info => "info",
            LevelFilter::Trace => "trace",
            LevelFilter::Debug => "debug",
        }
        .to_string()
    }
}

impl FromStr for LevelFilter {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "off" => Ok(Self::Off),
            "error" => Ok(Self::Error),
            "warn" => Ok(Self::Warn),
            "info" => Ok(Self::Info),
            "trace" => Ok(Self::Trace),
            "debug" => Ok(Self::Debug),
            _ => Err(anyhow!(
                "invalid format: allowed values: [off, error, warn, info, debug, trace]"
            )),
        }
    }
}
