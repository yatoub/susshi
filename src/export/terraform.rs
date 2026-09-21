//! Export Terraform inventory depuis la configuration susshi.
//!
//! Génère un fichier JSON compatible avec le provider Terraform `external`
//! ou utilisable comme inventaire statique via `locals` / `jsondecode`.
//!
//! Format :
//! ```json
//! {
//!   "servers": [
//!     {
//!       "name": "web-01",
//!       "host": "198.51.100.1",
//!       "user": "admin",
//!       "port": 22,
//!       "ssh_key": "~/.ssh/id_rsa",
//!       "group": "prod",
//!       "env": "production",
//!       "tags": ["web", "prod"]
//!     }
//!   ]
//! }
//! ```

use crate::config::ResolvedServer;

/// Génère un JSON Terraform depuis une liste de serveurs résolus.
pub fn to_terraform_json(servers: &[&ResolvedServer]) -> String {
    let items: Vec<serde_json::Value> = servers
        .iter()
        .map(|s| {
            serde_json::json!({
                "name": s.name,
                "host": s.host,
                "user": s.user,
                "port": s.port,
                "ssh_key": s.ssh_key,
                "group": s.group_name,
                "env": s.env_name,
                "namespace": s.namespace,
                "tags": s.tags,
            })
        })
        .collect();

    let root = serde_json::json!({ "servers": items });
    match serde_json::to_string_pretty(&root) {
        Ok(s) => s + "\n",
        Err(e) => {
            eprintln!("Erreur sérialisation JSON Terraform : {e}");
            std::process::exit(1);
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ConnectionMode, ResolvedServer};

    fn make_server(name: &str, host: &str, group: &str) -> ResolvedServer {
        ResolvedServer {
            namespace: String::new(),
            group_name: group.to_string(),
            env_name: String::new(),
            name: name.to_string(),
            host: host.to_string(),
            user: "admin".to_string(),
            port: 22,
            ssh_key: "~/.ssh/id_rsa".to_string(),
            ssh_cert: String::new(),
            ssh_agent_sock: String::new(),
            ssh_options: vec![],
            default_mode: ConnectionMode::Direct,
            jump_host: None,
            bastion_host: None,
            bastion_user: None,
            bastion_template: String::new(),
            use_system_ssh_config: false,
            probe_filesystems: vec![],
            tunnels: vec![],
            tags: vec!["web".to_string()],
            control_master: false,
            agent_forwarding: false,
            control_path: String::new(),
            control_persist: "10m".to_string(),
            pre_connect_hook: None,
            post_disconnect_hook: None,
            hook_timeout_secs: 5,
            wallix_group: None,
            wallix_account: "default".to_string(),
            wallix_protocol: "SSH".to_string(),
            wallix_auto_select: true,
            wallix_fail_if_menu_match_error: true,
            wallix_selection_timeout_secs: 8,
            wallix_direct: false,
            wallix_authorization: None,
            wallix_header_columns: vec![],
            notes: String::new(),
            production: false,
            confirm_production: false,
        }
    }

    #[test]
    fn empty_produces_empty_servers_array() {
        let json = to_terraform_json(&[]);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["servers"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn single_server_fields() {
        let srv = make_server("web-01", "198.51.100.1", "prod");
        let json = to_terraform_json(&[&srv]);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let s = &v["servers"][0];
        assert_eq!(s["name"], "web-01");
        assert_eq!(s["host"], "198.51.100.1");
        assert_eq!(s["user"], "admin");
        assert_eq!(s["port"], 22);
        assert_eq!(s["group"], "prod");
        assert_eq!(s["tags"][0], "web");
    }

    #[test]
    fn multiple_servers_all_present() {
        let s1 = make_server("alpha", "10.0.0.1", "g1");
        let s2 = make_server("beta", "10.0.0.2", "g2");
        let json = to_terraform_json(&[&s1, &s2]);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["servers"].as_array().unwrap().len(), 2);
        assert_eq!(v["servers"][0]["name"], "alpha");
        assert_eq!(v["servers"][1]["name"], "beta");
    }

    #[test]
    fn ssh_key_field_included() {
        let mut srv = make_server("key-srv", "10.0.0.3", "g");
        srv.ssh_key = "~/.ssh/prod_ed25519".into();
        let json = to_terraform_json(&[&srv]);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["servers"][0]["ssh_key"], "~/.ssh/prod_ed25519");
    }

    #[test]
    fn namespace_field_included() {
        let mut srv = make_server("ns-srv", "10.0.0.4", "g");
        srv.namespace = "MyNS".into();
        let json = to_terraform_json(&[&srv]);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["servers"][0]["namespace"], "MyNS");
    }

    #[test]
    fn env_field_included() {
        let mut srv = make_server("env-srv", "10.0.0.5", "g");
        srv.env_name = "staging".into();
        let json = to_terraform_json(&[&srv]);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["servers"][0]["env"], "staging");
    }

    #[test]
    fn output_ends_with_newline() {
        let srv = make_server("nl", "10.0.0.6", "g");
        let json = to_terraform_json(&[&srv]);
        assert!(json.ends_with('\n'));
    }
}
