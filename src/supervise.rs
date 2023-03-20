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

use crate::{
    cli::{StartArgs, UnsuperviseArgs},
    init::{ConfigFormat, Launcher},
};

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
const SYSTEMD_TEMPLATE: &str = r#"
[Unit]
Description=zeronsd for network {launcher.network_id}
Requires=zerotier-one.service
After=zerotier-one.service

[Service]
Type=simple
ExecStart={binpath} start -t {launcher.token} {{ if config }}-c {config} {{endif}}{{ if config_type_supplied }}--config-type {config_type} {{endif}}{{ if launcher.wildcard }}-w {{endif}}{{ if launcher.secret }}-s {launcher.secret} {{endif}}{{ if launcher.hosts }}-f {launcher.hosts} {{ endif }}{{ if launcher.domain }}-d {launcher.domain} {{ endif }}{launcher.network_id}
TimeoutStopSec=30
Restart=always

[Install]
WantedBy=default.target
"#;

#[cfg(target_os = "linux")]
const ALPINE_INIT_DIR: &str = "/etc/init.d";
#[cfg(target_os = "linux")]
const ALPINE_TEMPLATE: &str = r#"
#!/sbin/openrc-run

depend() \{
    need zerotier-one
    use network dns logger netmount
}

description="zeronsd for network {launcher.network_id}"
command="{binpath}"
command_args="start -t {launcher.token} {{ if config }}-c {config} {{endif}}{{ if config_type_supplied }}--config-type {config_type} {{endif}}{{ if launcher.wildcard }}-w {{endif}}{{ if launcher.secret }}-s {launcher.secret} {{endif}}{{ if launcher.hosts }}-f {launcher.hosts} {{ endif }}{{ if launcher.domain }}-d {launcher.domain} {{ endif }}{launcher.network_id}"
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
    <key>Label</key> <string>com.zerotier.nsd.{launcher.network_id}</string>

    <key>ProgramArguments</key>
    <array>
      <string>{binpath}</string>
      <string>start</string>
      <string>-t</string>
      <string>{launcher.token}</string>
      {{ if launcher.wildcard }}
      <string>-w</string>
      {{endif}}
      {{ if launcher.secret }}
      <string>-s</string>
      <string>{launcher.secret}</string>
      {{endif}}
      {{ if launcher.hosts }}
      <string>-f</string>
      <string>{launcher.hosts}</string>
      {{ endif }}
      {{ if launcher.domain }}
      <string>-d</string>
      <string>{launcher.domain}</string>
      {{ endif }}
      {{ if config }}
      <string>-c</string>
      <string>{config}</string>
      {{ endif }}
      {{ if config_type_supplied }}
      <string>--config-type</string>
      <string>{config_type}</string>
      {{ endif }}
      <string>{launcher.network_id}</string>
    </array>

    <key>UserName</key> <string>root</string>

    <key>RunAtLoad</key> <true/>

    <key>KeepAlive</key> <true/>

    <key>StandardErrorPath</key> <string>/var/log/zerotier/nsd/{launcher.network_id}.err</string>
    <key>StandardOutPath</key> <string>/var/log/zerotier/nsd/{launcher.network_id}.log</string>

  </dict>
    </plist>
"#;

#[derive(Serialize)]
pub struct Properties {
    pub launcher: Launcher,
    pub binpath: String,
    pub config: Option<PathBuf>,
    pub config_type: ConfigFormat,
    pub config_type_supplied: bool,
    pub distro: Option<String>,
}

impl From<StartArgs> for Properties {
    fn from(args: StartArgs) -> Self {
        let launcher: crate::init::Launcher = args.clone().into();

        // FIXME rewrite this to use a struct init later
        Self::new(launcher, args.config.as_deref(), args.config_type).unwrap()
    }
}

impl From<UnsuperviseArgs> for Properties {
    fn from(args: UnsuperviseArgs) -> Self {
        let l = Launcher {
            network_id: Some(args.network_id),
            ..Default::default()
        };

        Self::new(l, None, ConfigFormat::YAML).unwrap()
    }
}

impl Default for Properties {
    fn default() -> Self {
        Self {
            launcher: Launcher::default(),
            binpath: "zeronsd".to_string(),
            config: None,
            config_type: ConfigFormat::YAML,
            config_type_supplied: false,
            distro: None,
        }
    }
}

impl Properties {
    pub fn new(
        launcher: Launcher,
        config: Option<&'_ Path>,
        config_type: ConfigFormat,
    ) -> Result<Self, anyhow::Error> {
        let distro = if cfg!(target_os = "linux") {
            if let Ok(release) = std::fs::read_to_string(OS_RELEASE_FILE) {
                let id_regex = Regex::new(r#"\nID=(.+)\n"#)?;
                if let Some(caps) = id_regex.captures(&release) {
                    caps.get(1)
                        .map(|distro| distro.clone().as_str().to_string())
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
            binpath: String::from(std::env::current_exe()?.to_string_lossy()),
            config_type: config_type.clone(),
            config_type_supplied: config_type != ConfigFormat::YAML,
            config: config.map(|config| config.to_owned()),
            launcher,
        })
    }

    pub fn validate(&mut self) -> Result<(), anyhow::Error> {
        self.config = match self.config.clone() {
            Some(config) => match config.canonicalize() {
                Ok(res) => Some(res),
                Err(e) => return Err(anyhow!("Could not find token file: {}", e)),
            },
            None => None,
        };

        let token = self
            .launcher
            .token
            .clone()
            .expect("Could not find token file: {}")
            .canonicalize()?;

        let tstat = match std::fs::metadata(token.clone()) {
            Ok(ts) => ts,
            Err(e) => {
                return Err(anyhow!(
                    "Could not stat token file {}: {}",
                    token.display(),
                    e
                ))
            }
        };

        if !tstat.is_file() {
            return Err(anyhow!("Token file {} is not a file", token.display()));
        }

        if self
            .launcher
            .network_id
            .clone()
            .expect("network_id is not provided")
            .len()
            != 16
        {
            return Err(anyhow!("Network ID must be 16 characters"));
        }

        if let Some(hosts_file) = self.launcher.hosts.clone() {
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

            self.launcher.hosts = Some(hosts_file.canonicalize()?);
        }

        if let Some(domain) = self.launcher.domain.clone() {
            if domain.trim().is_empty() {
                return Err(anyhow!("Domain name cannot be empty"));
            }

            if let Err(e) = Name::parse(&domain, None) {
                return Err(anyhow!("Domain name is invalid: {}", e));
            }
        }

        if let Some(authtoken) = self.launcher.secret.clone() {
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
                    "launcher.secret file {} is not a file",
                    authtoken.display()
                ));
            }

            self.launcher.secret = Some(authtoken.canonicalize()?);
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
        let network_id = self
            .launcher
            .network_id
            .clone()
            .expect("network_id missing");
        match self.distro.as_deref() {
            Some("alpine") => format!("zeronsd-{}", network_id),
            _ => format!("zeronsd-{}.service", network_id),
        }
    }

    #[cfg(target_os = "macos")]
    fn service_name(&self) -> String {
        format!(
            "com.zerotier.nsd.{}.plist",
            self.launcher.network_id.as_ref().expect("network_id missing")
        )
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

            let network = self
                .launcher
                .network_id
                .clone()
                .expect("network_id missing");
            let systemd_help = format!("Don't forget to `systemctl daemon-reload`, `systemctl enable zeronsd-{}` and `systemctl start zeronsd-{}`.", network, network);
            let alpine_help = format!(
                "Don't forget to `rc-update add zeronsd-{}` and `rc-service zeronsd-{} start`",
                network, network
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
