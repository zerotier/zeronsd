/// code to tickle various supervisors to enable the `zeronsd supervise` command.
/// this code is hard to read but fundamentally launchd and systemd are controlled through a
/// library called `tinytemplate` and of course serde.
use std::path::PathBuf;

use anyhow::anyhow;
use log::info;
use serde::Serialize;
use tinytemplate::TinyTemplate;
use trust_dns_resolver::Name;

#[cfg(target_os = "windows")]
const SUPERVISE_SYSTEM_DIR: &str = "";
#[cfg(target_os = "windows")]
const SERVICE_TEMPLATE: &str = "";

#[cfg(target_os = "linux")]
const SUPERVISE_SYSTEM_DIR: &str = "/lib/systemd/system";

#[cfg(target_os = "linux")]
const SERVICE_TEMPLATE: &str = "
[Unit]
Description=zeronsd for network {network}
Requires=zerotier-one.service
After=zerotier-one.service

[Service]
Type=simple
ExecStart={binpath} start -t {token} {{ if wildcard_names }}-w {{endif}}{{ if authtoken }}-s {authtoken} {{endif}}{{ if hosts_file }}-f {hosts_file} {{ endif }}{{ if domain }}-d {domain} {{ endif }}{network}
TimeoutStopSec=30

[Install]
WantedBy=default.target
";

#[cfg(target_os = "macos")]
const SUPERVISE_SYSTEM_DIR: &str = "/Library/LaunchDaemons/";
#[cfg(target_os = "macos")]
const SERVICE_TEMPLATE: &str = r#"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
  <dict>
    <key>Label</key> <string>com.zerotier.nsd.{network}</string>

    <key>ProgramArguments</key>
    <array>
      <string>{binpath}</string>
      <string>start</string>
      <string>-t</string>
      <string>{token}</string>
      {{ if wildcard_names }}
      <string>-w</string>
      {{endif}}
      {{ if authtoken }}
      <string>-s</string>
      <string>{authtoken}</string>
      {{endif}}
      {{ if hosts_file }}
      <string>-f</string>
      <string>{hosts_file}</string>
      {{ endif }}
      {{ if domain }}
      <string>-d</string>
      <string>{domain}</string>
      {{ endif }}
      <string>{network}</string>
    </array>

    <key>UserName</key> <string>root</string>

    <key>RunAtLoad</key> <true/>

    <key>KeepAlive</key> <true/>

    <key>StandardErrorPath</key> <string>/var/log/zerotier/nsd/{network}.err</string>
    <key>StandardOutPath</key> <string>/var/log/zerotier/nsd/{network}.log</string>

  </dict>
    </plist>
"#;

#[derive(Serialize)]
pub struct Properties {
    pub binpath: String,
    pub domain: Option<String>,
    pub network: String,
    pub hosts_file: Option<String>,
    pub authtoken: Option<String>,
    pub token: String,
    pub wildcard_names: bool,
}

impl From<&clap::ArgMatches<'_>> for Properties {
    fn from(args: &clap::ArgMatches<'_>) -> Self {
        Self::new(
            args.value_of("domain"),
            args.value_of("NETWORK_ID"),
            args.value_of("file").clone(),
            args.value_of("secret_file"),
            args.value_of("token_file"),
            args.is_present("wildcard"),
        )
        .unwrap()
    }
}

impl Default for Properties {
    fn default() -> Self {
        Self {
            wildcard_names: false,
            binpath: "zeronsd".to_string(),
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
        domain: Option<&'_ str>,
        network: Option<&'_ str>,
        hosts_file: Option<&'_ str>,
        authtoken: Option<&'_ str>,
        token: Option<&'_ str>,
        wildcard_names: bool,
    ) -> Result<Self, anyhow::Error> {
        Ok(Self {
            wildcard_names,
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

    pub fn supervise_template(&self) -> Result<String, anyhow::Error> {
        let mut t = TinyTemplate::new();
        t.add_template("supervise", SERVICE_TEMPLATE)?;
        match t.render("supervise", self) {
            Ok(x) => Ok(x),
            Err(e) => Err(anyhow!(e)),
        }
    }

    #[cfg(target_os = "windows")]
    fn service_name(&self) -> String {
        return String::new();
    }

    #[cfg(target_os = "linux")]
    fn service_name(&self) -> String {
        format!("zeronsd-{}.service", self.network)
    }

    #[cfg(target_os = "macos")]
    fn service_name(&self) -> String {
        format!("com.zerotier.nsd.{}.plist", self.network)
    }

    fn service_path(&self) -> PathBuf {
        PathBuf::from(SUPERVISE_SYSTEM_DIR).join(self.service_name())
    }

    pub fn install_supervisor(&mut self) -> Result<(), anyhow::Error> {
        self.validate()?;

        if cfg!(target_os = "linux") {
            let template = self.supervise_template()?;
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

            info!(
                "Service definition written to {}.\nDon't forget to `systemctl daemon-reload` and `systemctl enable zeronsd-{}`",
                service_path.to_str().expect("Could not coerce service path to string"),
                self.network,
            );
        } else if cfg!(target_os = "macos") {
            let template = self.supervise_template()?;
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

            info!(
                "Service definition written to {}.\nTo start the service, run:\nsudo launchctl load {}",
                service_path.to_str().expect("Could not coerce service path to string"),
                service_path.to_str().expect("Could not coerce service path to string")
            );
        } else {
            return Err(anyhow!("Your platform is not supported for this command"));
        }
        Ok(())
    }

    pub fn uninstall_supervisor(&self) -> Result<(), anyhow::Error> {
        if cfg!(target_os = "linux") {
            match std::fs::remove_file(self.service_path()) {
                Ok(_) => {}
                Err(e) => {
                    return Err(anyhow!(
                        "Could not uninstall supervisor unit file ({}): {}",
                        self.service_path()
                            .to_str()
                            .expect("Could not coerce service path to string"),
                        e,
                    ))
                }
            };
        } else if cfg!(target_os = "macos") {
            match std::fs::remove_file(self.service_path()) {
                Ok(_) => {}
                Err(e) => {
                    return Err(anyhow!(
                        "Could not uninstall supervisor unit file ({}): {}",
                        self.service_path()
                            .to_str()
                            .expect("Could not coerce service path to string"),
                        e,
                    ))
                }
            };

            info!(
                "Service definition removed from {}.\nDon't forget to stop it:\nsudo launchctl remove {}",
                self.service_path().to_str().expect("Could not coerce service path to string"),
                self.service_name().replace(".plist", "")
            );
        } else {
            return Err(anyhow!("Your platform is not supported for this command"));
        }
        Ok(())
    }
}
