use zerotier_central_api::models::{Member, MemberConfig};

// monkeypatches to Member
pub trait MemberUtil {
    // set some member defaults for testing
    fn set_defaults(&mut self, network_id: String, identity: String);
}

// monkeypatches to MemberConfig
pub trait MemberConfigUtil {
    fn set_ip_assignments(&mut self, ips: Vec<&str>);
    fn set_defaults(&mut self, identity: String);
}

impl MemberUtil for Member {
    fn set_defaults(&mut self, network_id: String, identity: String) {
        self.node_id = Some(identity.clone());
        self.network_id = Some(network_id);
        let mut mc = MemberConfig::new();
        mc.set_defaults(identity);
        self.config = Some(Box::new(mc));
    }
}

impl MemberConfigUtil for MemberConfig {
    fn set_ip_assignments(&mut self, ips: Vec<&str>) {
        self.ip_assignments = Some(ips.into_iter().map(|s| s.to_string()).collect())
    }

    fn set_defaults(&mut self, identity: String) {
        self.v_rev = None;
        self.v_major = None;
        self.v_proto = None;
        self.v_minor = None;
        self.tags = None;
        self.revision = None;
        self.no_auto_assign_ips = Some(false);
        self.last_authorized_time = None;
        self.last_deauthorized_time = None;
        self.id = None;
        self.creation_time = None;
        self.capabilities = None;
        self.ip_assignments = None;
        self.authorized = Some(true);
        self.active_bridge = None;
        self.identity = Some(identity);
    }
}
