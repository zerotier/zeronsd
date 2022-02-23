/// code to tickle various supervisors to enable the `zeronsd supervise` command.
/// this code is hard to read but fundamentally launchd and systemd are controlled through a
/// library called `tinytemplate` and of course serde.
use std::path::{Path, PathBuf};

use anyhow::anyhow;
use regex::Regex;
use serde::Serialize;
use tinytemplate::TinyTemplate;
use trust_dns_resolver::Name;

#[cfg(target_os = "linux")]
use std::os::unix::fs::PermissionsExt;

use crate::cli::{StartArgs, UnsuperviseArgs};

#[cfg(target_os = "windows")]
const SUPERVISE_SYSTEM_DIR: &str = "";
#[cfg(target_os = "windows")]
const SERVICE_TEMPLATE: &str = "";
#[cfg(target_os = "windows")]
const OS_RELEASE_FILE: &str = "";
#[cfg(target_os = "windows")]
const ALPINE_INIT_DIR: &str = "";

#[cfg(target_os = "linux")]
const SUPERVISE_SYSTEM_DIR: &str = "/lib/systemd/system";
#[cfg(target_os = "linux")]
const OS_RELEASE_FILE: &str = "/etc/os-release";

#[cfg(target_os = "linux")]
const SYSTEMD_TEMPLATE: &str = "
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

#[cfg(target_os = "linux")]
const ALPINE_INIT_DIR: &str = "/etc/init.d";
#[cfg(target_os = "linux")]
const ALPINE_TEMPLATE: &str = r#"
#!/sbin/openrc-run

depend() \{
    need zerotier-one
    use network dns logger netmount
}

description="zeronsd for network {network}"
command="{binpath}"
command_args="start -t {token} {{ if wildcard_names }}-w {{endif}}{{ if authtoken }}-s {authtoken} {{endif}}{{ if hosts_file }}-f {hosts_file} {{ endif }}{{ if domain }}-d {domain} {{ endif }}{network}"
command_background="yes"
pidfile="/run/$RC_SVCNAME.pid"
"#;

#[cfg(target_os = "macos")]
const OS_RELEASE_FILE: &str = "";
#[cfg(target_os = "macos")]
const ALPINE_INIT_DIR: &str = "";
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
    pub hosts_file: Option<PathBuf>,
    pub authtoken: Option<PathBuf>,
    pub token: PathBuf,
    pub wildcard_names: bool,
    pub distro: Option<String>,
}

impl From<StartArgs> for Properties {
    fn from(args: StartArgs) -> Self {
        let args: crate::init::Launcher = args.into();

        Self::new(
            args.domain.as_deref(),
            &args.network_id,
            args.hosts.as_deref(),
            args.secret.as_deref(),
            args.token.as_deref(),
            args.wildcard,
        )
        .unwrap()
    }
}

impl From<UnsuperviseArgs> for Properties {
    fn from(args: UnsuperviseArgs) -> Self {
        Self::new(None, &args.network_id, None, None, None, false).unwrap()
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
            token: PathBuf::new(),
            distro: None,
        }
    }
}

impl<'a> Properties {
    pub fn new(
        domain: Option<&'_ str>,
        network: &'_ str,
        hosts_file: Option<&'_ Path>,
        authtoken: Option<&'_ Path>,
        token: Option<&'_ Path>,
        wildcard_names: bool,
    ) -> Result<Self, anyhow::Error> {
        let distro = if cfg!(target_os = "linux") {
            if let Ok(release) = std::fs::read_to_string(OS_RELEASE_FILE) {
                let id_regex = Regex::new(r#"\nID=(.+)\n"#)?;
                if let Some(caps) = id_regex.captures(&release) {
                    if let Some(distro) = caps.get(1) {
                        Some(distro.clone().as_str().to_string())
                    } else {
                        None
                    }
                } else {
                    return Err(anyhow!("Could not determine Linux distribution; you'll need to configure supervision manually. Sorry!"));
                }
            } else {
                return Err(anyhow!("Could not determine Linux distribution; you'll need to configure supervision manually. Sorry!"));
            }
        } else {
            None
        };

        Ok(Self {
            distro,
            wildcard_names,
            binpath: String::from(std::env::current_exe()?.to_string_lossy()),
            // make this garbage a macro later
            domain: match domain {
                Some(domain) => Some(String::from(domain)),
                None => None,
            },
            network: network.into(),
            hosts_file: match hosts_file {
                Some(hosts_file) => Some(hosts_file.to_owned()),
                None => None,
            },
            authtoken: match authtoken {
                Some(authtoken) => Some(authtoken.to_owned()),
                None => None,
            },
            token: token.unwrap_or(Path::new("")).to_owned(),
        })
    }

    pub fn validate(&mut self) -> Result<(), anyhow::Error> {
        self.token = match self.token.canonicalize() {
            Ok(res) => res,
            Err(e) => return Err(anyhow!("Could not find token file: {}", e)),
        };

        let tstat = match std::fs::metadata(self.token.clone()) {
            Ok(ts) => ts,
            Err(e) => {
                return Err(anyhow!(
                    "Could not stat token file {}: {}",
                    self.token.display(),
                    e
                ))
            }
        };

        if !tstat.is_file() {
            return Err(anyhow!("Token file {} is not a file", self.token.display()));
        }

        if self.network.len() != 16 {
            return Err(anyhow!("Network ID must be 16 characters"));
        }

        if let Some(hosts_file) = self.hosts_file.clone() {
            let hstat = match std::fs::metadata(hosts_file.clone()) {
                Ok(hs) => hs,
                Err(e) => {
                    return Err(anyhow!(
                        "Could not stat hosts file {}: {}",
                        hosts_file.display(),
                        e
                    ))
                }
            };

            if !hstat.is_file() {
                return Err(anyhow!("Hosts file {} is not a file", hosts_file.display()));
            }

            self.hosts_file = Some(hosts_file.canonicalize()?);
        }

        if let Some(domain) = self.domain.clone() {
            if domain.trim().len() == 0 {
                return Err(anyhow!("Domain name cannot be empty"));
            }

            if let Err(e) = Name::parse(&domain, None) {
                return Err(anyhow!("Domain name is invalid: {}", e));
            }
        }

        if let Some(authtoken) = self.authtoken.clone() {
            let hstat = match std::fs::metadata(authtoken.clone()) {
                Ok(hs) => hs,
                Err(e) => {
                    return Err(anyhow!(
                        "Could not stat authtoken file {}: {}",
                        authtoken.display(),
                        e
                    ))
                }
            };

            if !hstat.is_file() {
                return Err(anyhow!(
                    "authtoken file {} is not a file",
                    authtoken.display()
                ));
            }

            self.authtoken = Some(authtoken.canonicalize()?);
        }

        Ok(())
    }

    pub fn supervise_template(&self) -> Result<String, anyhow::Error> {
        let template = self.get_service_template();

        let mut t = TinyTemplate::new();
        t.add_template("supervise", template)?;
        match t.render("supervise", self) {
            Ok(x) => Ok(x),
            Err(e) => Err(anyhow!(e)),
        }
    }

    #[cfg(target_os = "linux")]
    fn get_service_template(&self) -> &str {
        match self.distro.as_deref() {
            Some("alpine") => ALPINE_TEMPLATE.trim(),
            _ => SYSTEMD_TEMPLATE,
        }
    }

    #[cfg(target_os = "windows")]
    fn get_service_template(&self) -> &str {
        return SERVICE_TEMPLATE;
    }

    #[cfg(target_os = "macos")]
    fn get_service_template(&self) -> &str {
        return SERVICE_TEMPLATE;
    }

    #[cfg(target_os = "windows")]
    fn service_name(&self) -> String {
        return String::new();
    }

    #[cfg(target_os = "linux")]
    fn service_name(&self) -> String {
        match self.distro.as_deref() {
            Some("alpine") => format!("zeronsd-{}", self.network),
            _ => format!("zeronsd-{}.service", self.network),
        }
    }

    #[cfg(target_os = "macos")]
    fn service_name(&self) -> String {
        format!("com.zerotier.nsd.{}.plist", self.network)
    }

    fn service_path(&self) -> PathBuf {
        let dir = match self.distro.as_deref() {
            Some("alpine") => ALPINE_INIT_DIR,
            _ => SUPERVISE_SYSTEM_DIR,
        };
        PathBuf::from(dir).join(self.service_name())
    }

    pub fn install_supervisor(&mut self) -> Result<(), anyhow::Error> {
        self.validate()?;

        if cfg!(target_os = "linux") {
            #[cfg(target_os = "linux")]
            let executable = self.distro.as_deref() == Some("alpine");

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

            #[cfg(target_os = "linux")]
            if executable {
                let mut perms = std::fs::metadata(service_path.clone())?.permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(service_path.clone(), perms)?;
            }

            let systemd_help = format!("Don't forget to `systemctl daemon-reload`, `systemctl enable zeronsd-{}` and `systemctl start zeronsd-{}`.", self.network, self.network);
            let alpine_help = format!(
                "Don't forget to `rc-update add zeronsd-{}` and `rc-service zeronsd-{} start`",
                self.network, self.network
            );

            let help = match self.distro.as_deref() {
                Some("alpine") => alpine_help,
                _ => systemd_help,
            };

            eprintln!(
                "Service definition written to {}.\n{}",
                service_path
                    .to_str()
                    .expect("Could not coerce service path to string"),
                help,
            );
        } else if cfg!(target_os = "macos") {
            let template = self.supervise_template()?;
            let service_path = self.service_path();

            match std::fs::write(&service_path, template) {
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

            eprintln!(
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
            eprintln!(
                "Service definition removed from {}.\nDon't forget to reload systemd:\nsudo systemctl daemon-reload",
                self.service_path().to_str().expect("Could not coerce service path to string"),
            );
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

            eprintln!(
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
