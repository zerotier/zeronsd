/// functionality to deal with the handling of /etc/hosts formatted files
use log::warn;
use std::{collections::HashMap, net::IpAddr, path::PathBuf, str::FromStr};
use trust_dns_server::client::rr::Name;

use crate::utils::ToHostname;

pub(crate) type HostsFile = HashMap<IpAddr, Vec<Name>>;

const WHITESPACE_SPLIT: &str = r"\s+";
const COMMENT_MATCH: &str = r"^\s*#";

/// Parses an /etc/hosts-formatted file into a mapping of ip -> [name]. Used to populate the
/// authority.
pub(crate) fn parse_hosts(
    hosts_file: Option<PathBuf>,
    domain_name: Name,
) -> Result<HostsFile, std::io::Error> {
    let mut input: HostsFile = HashMap::new();

    if hosts_file.is_none() {
        return Ok(input);
    }

    let whitespace = regex::Regex::new(WHITESPACE_SPLIT).unwrap();
    let comment = regex::Regex::new(COMMENT_MATCH).unwrap();
    let content = std::fs::read_to_string(hosts_file.clone().unwrap())?;

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }

        // after whitespace is ruled out as the only thing on the line, the line is split by ..
        // whitespace and the parts iterated.
        let mut ary = whitespace.split(line);

        // the first item will be the ip
        if let Some(ip) = ary.next() {
            // technically we're still matching the head of the line at this point. if it's a
            // comment, bail.
            if comment.is_match(ip) {
                continue;
            }

            // ensure we have an IP, again, this is still the first field.
            match IpAddr::from_str(ip) {
                Ok(parsed_ip) => {
                    // now that we have the ip, it's all names now.
                    let mut v: Vec<Name> = Vec::new();

                    // continue to iterate over the hosts. If we encounter a comment, stop
                    // processing.
                    for host in ary.take_while(|h| !comment.is_match(h)) {
                        let fqdn = match host.to_fqdn(domain_name.clone()) {
                            Ok(fqdn) => Some(fqdn),
                            Err(e) => {
                                warn!("Invalid host {}: {:?}", host, e);
                                None
                            }
                        };

                        if let Some(fqdn) = fqdn {
                            v.push(fqdn)
                        }
                    }

                    // if we have a valid ip in the collection already, append, don't clobber
                    // it.
                    input.entry(parsed_ip).or_default().extend(v);
                }
                Err(e) => {
                    warn!("Couldn't parse {}: {}", ip, e);
                }
            }
        }
    }

    Ok(input)
}
