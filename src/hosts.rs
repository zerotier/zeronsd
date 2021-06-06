use std::{collections::HashMap, io::Write, net::IpAddr, str::FromStr};
use trust_dns_server::client::rr::Name;

use crate::utils::ToHostname;

pub type HostsFile = HashMap<IpAddr, Vec<Name>>;

const WHITESPACE_SPLIT: &str = r"\s+";
const COMMENT_MATCH: &str = r"^\s*#";

pub fn parse_hosts(
    hosts_file: Option<String>,
    domain_name: Name,
) -> Result<HostsFile, std::io::Error> {
    let mut input: HostsFile = HashMap::new();

    if let None = hosts_file {
        return Ok(input);
    }

    let whitespace = regex::Regex::new(WHITESPACE_SPLIT).unwrap();
    let comment = regex::Regex::new(COMMENT_MATCH).unwrap();
    let content = std::fs::read_to_string(hosts_file.clone().unwrap())?;

    for line in content.lines() {
        if line.trim().len() == 0 {
            continue;
        }

        let mut ary = whitespace.split(line);

        // the first item will be the ip
        match ary.next() {
            Some(ip) => {
                if comment.is_match(ip) {
                    continue;
                }

                match IpAddr::from_str(ip) {
                    Ok(parsed_ip) => {
                        let mut v: Vec<Name> = Vec::new();

                        // continue to iterate over the hosts
                        for host in ary.take_while(|h| !comment.is_match(h)) {
                            let fqdn = match host.to_fqdn(domain_name.clone()) {
                                Ok(fqdn) => Some(fqdn),
                                Err(e) => {
                                    eprintln!("Invalid host {}: {:?}", host, e);
                                    None
                                }
                            };

                            if let Some(fqdn) = fqdn {
                                v.push(fqdn)
                            }
                        }

                        input.insert(parsed_ip, v);
                    }
                    Err(e) => {
                        writeln!(std::io::stderr().lock(), "Couldn't parse {}: {}", ip, e)?;
                    }
                }
            }
            None => {}
        }
    }

    Ok(input)
}
