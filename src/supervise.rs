/// code to tickle various supervisors to enable the `zeronsd supervise` command.
/// this code is hard to read but fundamentally launchd and systemd are controlled through a
/// library called `tinytemplate` and of course serde.
use std::path::PathBuf;

use anyhow::anyhow;
use log::info;
use regex::Regex;
use serde::Serialize;
use tinytemplate::TinyTemplate;
use trust_dns_resolver::Name;

#[cfg(target_os = "linux")]
use std::os::unix::fs::PermissionsExt;

#[cfg(target_os = "windows")]
const SUPERVISE_SYSTEM_DIR: &str = "";
#[cfg(target_os = "windows")]
const SERVICE_TEMPLATE: &str = "";

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

    pub fn supervise_template(&self, distro: Option<&str>) -> Result<String, anyhow::Error> {
        let template = self.get_service_template(distro);

        let mut t = TinyTemplate::new();
        t.add_template("supervise", template)?;
        match t.render("supervise", self) {
            Ok(x) => Ok(x),
            Err(e) => Err(anyhow!(e)),
        }
    }

    #[cfg(target_os = "linux")]
    fn get_service_template(&self, distro: Option<&str>) -> &str {
        match distro {
            Some(s) => match s {
                "alpine" => ALPINE_TEMPLATE,
                _ => SYSTEMD_TEMPLATE,
            },
            None => SYSTEMD_TEMPLATE,
        }
    }

    #[cfg(target_os = "windows")]
    fn get_service_template(&self, distro: Option<&str>) -> &str {
        return SERVICE_TEMPLATE;
    }

    #[cfg(target_os = "macos")]
    fn get_service_template(&self, distro: Option<&str>) -> &str {
        return SERVICE_TEMPLATE;
    }

    #[cfg(target_os = "windows")]
    fn service_name(&self) -> String {
        return String::new();
    }

    #[cfg(target_os = "linux")]
    fn service_name(&self, distro: Option<&str>) -> String {
        match distro {
            Some(s) => match s {
                "alpine" => format!("zeronsd-{}", self.network),
                _ => format!("zeronsd-{}.service", self.network),
            },
            None => format!("zeronsd-{}.service", self.network),
        }
    }

    #[cfg(target_os = "macos")]
    fn service_name(&self) -> String {
        format!("com.zerotier.nsd.{}.plist", self.network)
    }

    fn service_path(&self, distro: Option<&str>) -> PathBuf {
        let dir = match distro {
            Some(s) => match s {
                "alpine" => ALPINE_INIT_DIR,
                _ => SUPERVISE_SYSTEM_DIR,
            },
            None => SUPERVISE_SYSTEM_DIR,
        };
        PathBuf::from(dir).join(self.service_name(distro))
    }

    pub fn install_supervisor(&mut self) -> Result<(), anyhow::Error> {
        self.validate()?;

        if cfg!(target_os = "linux") {
            if let Ok(release) = std::fs::read_to_string(OS_RELEASE_FILE) {
                let id_regex = Regex::new(r#"\nID=(.+)\n"#)?;
                if let Some(caps) = id_regex.captures(&release) {
                    if let Some(distro) = caps.get(1) {
                        let distro = distro.as_str();

                        let executable = match distro {
                            "alpine" => true,
                            _ => false,
                        };

                        let template = self.supervise_template(Some(distro))?;
                        let service_path = self.service_path(Some(distro));

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

                        if executable {
                            let mut perms = std::fs::metadata(service_path.clone())?.permissions();
                            perms.set_mode(0755);
                            std::fs::set_permissions(service_path.clone(), perms)?;
                        }

                        let systemd_help = format!("Don't forget to `systemctl daemon-reload`, `systemctl enable zeronsd-{}` and `systemctl start zeronsd-{}`.", self.network, self.network);
                        let alpine_help = format!(
                            "Don't to `rc-update add zeronsd-{}` and `rc-service start zeronsd-{}`",
                            self.network, self.network
                        );

                        let help = match distro {
                            "alpine" => alpine_help,
                            _ => systemd_help,
                        };

                        info!(
                            "Service definition written to {}.\n{}",
                            service_path
                                .to_str()
                                .expect("Could not coerce service path to string"),
                            help,
                        );
                    } else {
                        return Err(anyhow!("Could not determine Linux distribution; you'll need to configure supervision manually. Sorry!"));
                    }
                } else {
                    return Err(anyhow!("Could not determine Linux distribution; you'll need to configure supervision manually. Sorry!"));
                }
            }
        } else if cfg!(target_os = "macos") {
            let template = self.supervise_template(None)?;
            let service_path = self.service_path(None);

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
            match std::fs::remove_file(self.service_path(None)) {
                Ok(_) => {}
                Err(e) => {
                    return Err(anyhow!(
                        "Could not uninstall supervisor unit file ({}): {}",
                        self.service_path(None)
                            .to_str()
                            .expect("Could not coerce service path to string"),
                        e,
                    ))
                }
            };
        } else if cfg!(target_os = "macos") {
            match std::fs::remove_file(self.service_path(None)) {
                Ok(_) => {}
                Err(e) => {
                    return Err(anyhow!(
                        "Could not uninstall supervisor unit file ({}): {}",
                        self.service_path(None)
                            .to_str()
                            .expect("Could not coerce service path to string"),
                        e,
                    ))
                }
            };

            info!(
                "Service definition removed from {}.\nDon't forget to stop it:\nsudo launchctl remove {}",
                self.service_path(None).to_str().expect("Could not coerce service path to string"),
                self.service_name(None).replace(".plist", "")
            );
        } else {
            return Err(anyhow!("Your platform is not supported for this command"));
        }
        Ok(())
    }
}
