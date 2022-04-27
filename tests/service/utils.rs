use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use zeronsd::utils::authtoken_path;

pub fn randstring(len: u8) -> String {
    "zeronsd-test-".to_string()
        + (0..len)
            .map(|_| (rand::random::<u8>() % 26) + 'a' as u8)
            .map(|c| {
                if rand::random::<bool>() {
                    (c as char).to_ascii_uppercase()
                } else {
                    c as char
                }
            })
            .map(|c| c.to_string())
            .collect::<Vec<String>>()
            .join("")
            .as_str()
}

// extract a network definiton from testdata. templates in a new name.
pub fn network_definition(
    name: String,
) -> Result<HashMap<String, serde_json::Value>, anyhow::Error> {
    let mut res: HashMap<String, serde_json::Value> = serde_json::from_reader(
        std::fs::File::open(format!("testdata/networks/{}.json", name))?,
    )?;

    if let serde_json::Value::Object(config) = res.clone().get("config").unwrap() {
        let mut new_config = config.clone();
        new_config.insert(
            "name".to_string(),
            serde_json::Value::String(randstring(30)),
        );

        res.insert("config".to_string(), serde_json::Value::Object(new_config));
    }

    Ok(res)
}

// returns the public identity of this instance of zerotier
pub async fn get_identity(client: &zerotier_one_api::Client) -> Result<String, anyhow::Error> {
    let status = client.get_status().await?;

    Ok(status
        .to_owned()
        .public_identity
        .unwrap()
        .splitn(3, ":")
        .nth(0)
        .unwrap()
        .to_owned())
}

// unpack the authtoken based on what we're passed
pub fn get_authtoken(or: Option<&str>) -> Result<String, anyhow::Error> {
    Ok(std::fs::read_to_string(authtoken_path(
        or.map(|c| Path::new(c)),
    ))?)
}

pub enum HostsType {
    Path(&'static str),
    Fixture(&'static str),
    None,
}

pub fn format_hosts_file(hosts: HostsType) -> Option<PathBuf> {
    match hosts {
        HostsType::Fixture(hosts) => {
            Some(Path::new(&format!("{}/{}", zeronsd::utils::TEST_HOSTS_DIR, hosts)).to_path_buf())
        }
        HostsType::Path(hosts) => Some(Path::new(hosts).to_path_buf()),
        HostsType::None => None,
    }
}
