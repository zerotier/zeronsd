use std::path::PathBuf;

use anyhow::anyhow;
use serde::Serialize;
use tinytemplate::TinyTemplate;
use trust_dns_resolver::Name;

const SYSTEMD_SYSTEM_DIR: &str = "/lib/systemd/system";
const SYSTEMD_UNIT: &str = "
[Unit]
Description=zeronsd for network {network}
Requires=zerotier-one.service
After=zerotier-one.service

[Service]
Type=simple
ExecStart={binpath} start -t {token} {{ if authtoken }}-s {authtoken} {{endif}}{{ if hosts_file }}-f {hosts_file} {{ endif }}{{ if domain }}-d {domain} {{ endif }}{network}
TimeoutStopSec=30

[Install]
WantedBy=default.target
";

#[derive(Serialize)]
pub struct Properties {
    pub binpath: String,
    pub domain: Option<String>,
    pub network: String,
    pub hosts_file: Option<String>,
    pub authtoken: Option<String>,
    pub token: String,
}

impl Default for Properties {
    fn default() -> Self {
        Self {
            binpath: String::from("zeronsd"),
            domain: None,
            network: String::new(),
            hosts_file: None,
            authtoken: None,
            token: String::new(),
        }
    }
}

impl<'a> Properties {
    pub fn new(
        domain: Option<&'a str>,
        network: Option<&'a str>,
        hosts_file: Option<&'a str>,
        authtoken: Option<&'a str>,
        token: Option<&'a str>,
    ) -> Result<Self, anyhow::Error> {
        Ok(Self {
            binpath: String::from(std::env::current_exe()?.to_string_lossy()),
            // make this garbage a macro later
            domain: match domain {
                Some(domain) => Some(String::from(domain)),
                None => None,
            },
            network: network.unwrap().into(),
            hosts_file: match hosts_file {
                Some(hosts_file) => Some(String::from(hosts_file)),
                None => None,
            },
            authtoken: match authtoken {
                Some(authtoken) => Some(String::from(authtoken)),
                None => None,
            },
            token: String::from(token.unwrap_or_default()),
        })
    }

    pub fn validate(&mut self) -> Result<(), anyhow::Error> {
        let tstat = match std::fs::metadata(self.token.clone()) {
            Ok(ts) => ts,
            Err(e) => return Err(anyhow!("Could not stat token file {}: {}", self.token, e)),
        };

        if !tstat.is_file() {
            return Err(anyhow!("Token file {} is not a file", self.token));
        }

        self.token = String::from(std::fs::canonicalize(self.token.clone())?.to_string_lossy());

        if self.network.len() != 16 {
            return Err(anyhow!("Network ID must be 16 characters"));
        }

        if let Some(hosts_file) = self.hosts_file.clone() {
            let hstat = match std::fs::metadata(hosts_file.clone()) {
                Ok(hs) => hs,
                Err(e) => return Err(anyhow!("Could not stat hosts file {}: {}", hosts_file, e)),
            };

            if !hstat.is_file() {
                return Err(anyhow!("Hosts file {} is not a file", hosts_file));
            }

            self.hosts_file = Some(
                std::fs::canonicalize(hosts_file)?
                    .to_string_lossy()
                    .to_string(),
            );
        }

        if let Some(domain) = self.domain.clone() {
            if domain.trim().len() == 0 {
                return Err(anyhow!("Domain name cannot be empty"));
            }

            match Name::parse(domain.as_str(), None) {
                Ok(_) => {}
                Err(e) => return Err(anyhow!("Domain name is invalid: {}", e)),
            }
        }

        if let Some(authtoken) = self.authtoken.clone() {
            let hstat = match std::fs::metadata(authtoken.clone()) {
                Ok(hs) => hs,
                Err(e) => {
                    return Err(anyhow!(
                        "Could not stat authtoken file {}: {}",
                        authtoken,
                        e
                    ))
                }
            };

            if !hstat.is_file() {
                return Err(anyhow!("authtoken file {} is not a file", authtoken));
            }

            self.authtoken = Some(
                std::fs::canonicalize(authtoken)?
                    .to_string_lossy()
                    .to_string(),
            );
        }

        Ok(())
    }

    pub fn systemd_template(&self) -> Result<String, anyhow::Error> {
        let mut t = TinyTemplate::new();
        t.add_template("systemd", SYSTEMD_UNIT)?;
        match t.render("systemd", self) {
            Ok(x) => Ok(x),
            Err(e) => Err(anyhow!(e)),
        }
    }

    fn service_name(&self) -> String {
        format!("zeronsd-{}.service", self.network)
    }

    fn service_path(&self) -> PathBuf {
        PathBuf::from(SYSTEMD_SYSTEM_DIR).join(self.service_name())
    }

    pub fn install_supervisor(&mut self) -> Result<(), anyhow::Error> {
        self.validate()?;

        if cfg!(target_os = "linux") {
            let template = self.systemd_template()?;
            let service_path = self.service_path();

            match std::fs::write(service_path.clone(), template) {
                Ok(_) => {}
                Err(e) => {
                    return Err(anyhow!(
                        "Could not write the template {}; are you root? ({})",
                        service_path
                            .to_str()
                            .expect("Could not coerce service path to string"),
                        e,
                    ))
                }
            };

            println!(
                "Service definition written to {}.\nDon't forget to `systemctl daemon-reload` and `systemctl enable zeronsd-{}`",
                service_path.to_str().expect("Could not coerce service path to string"),
                self.network,
            );
        } else {
            return Err(anyhow!("Your platform is not supported for this command"));
        }
        Ok(())
    }

    pub fn uninstall_supervisor(&self) -> Result<(), anyhow::Error> {
        if cfg!(target_os = "linux") {
            match std::fs::remove_file(self.service_path()) {
                Ok(_) => Ok(()),
                Err(e) => Err(anyhow!(
                    "Could not uninstall supervisor unit file ({}): {}",
                    self.service_path()
                        .to_str()
                        .expect("Could not coerce service path to string"),
                    e
                )),
            }
        } else {
            Err(anyhow!("Your platform is not supported for this command"))
        }
    }
}
