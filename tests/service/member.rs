use zerotier_central_api::types::{Member, MemberConfig};

// monkeypatches to Member
pub trait MemberUtil {
    // set some member defaults for testing
    fn new(network_id: String, identity: String) -> Self;
}

// monkeypatches to MemberConfig
pub trait MemberConfigUtil {
    fn set_ip_assignments(&mut self, ips: Vec<&str>);
    fn new(identity: String) -> Self;
}

impl MemberUtil for Member {
    fn new(network_id: String, identity: String) -> Self {
        let mut s = Self {
            protocol_version: None,
            supports_rules_engine: None,
            physical_address: None,
            name: None,
            last_online: None,
            id: None,
            hidden: None,
            description: None,
            controller_id: None,
            config: None,
            clock: None,
            client_version: None,
            node_id: Some(identity.clone()),
            network_id: Some(network_id),
        };

        s.config = Some(MemberConfig::new(identity));
        s
    }
}

impl MemberConfigUtil for MemberConfig {
    fn set_ip_assignments(&mut self, ips: Vec<&str>) {
        self.ip_assignments = Some(ips.into_iter().map(|s| s.to_string()).collect())
    }

    fn new(identity: String) -> Self {
        Self {
            v_rev: None,
            v_major: None,
            v_proto: None,
            v_minor: None,
            tags: Some(Vec::new()),
            revision: None,
            no_auto_assign_ips: Some(false),
            last_authorized_time: None,
            last_deauthorized_time: None,
            id: None,
            creation_time: None,
            capabilities: Some(Vec::new()),
            ip_assignments: Some(Vec::new()),
            authorized: Some(true),
            active_bridge: None,
            identity: Some(identity),
        }
    }
}
