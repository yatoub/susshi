//! Export Nmap target list depuis la configuration susshi.
//!
//! Génère une liste de cibles au format Nmap (`-iL`), une par ligne.
//! Si le port est différent de 22, on l'inclut au format `host:port`
//! (les scanners Nmap utilisent `-p` séparément, mais ce format reste lisible
//! par d'autres outils comme `masscan --include-file`).
//!
//! Exemple de sortie :
//! ```text
//! # susshi nmap targets — 3 serveur(s)
//! # [prod] web-01
//! 198.51.100.1
//! # [prod] db-01 (port 2222)
//! 198.51.100.2
//! # [staging] web-stg
//! 198.51.100.20
//! ```

use crate::config::ResolvedServer;

/// Génère une liste de cibles Nmap depuis une liste de serveurs résolus.
pub fn to_nmap_targets(servers: &[&ResolvedServer]) -> String {
    let mut out = format!("# susshi nmap targets — {} serveur(s)\n", servers.len());
    for srv in servers {
        let group_env = if srv.env_name.is_empty() {
            srv.group_name.clone()
        } else {
            format!("{}/{}", srv.group_name, srv.env_name)
        };
        if srv.port != 22 {
            out.push_str(&format!(
                "# [{}] {} (port {})\n{}\n",
                group_env, srv.name, srv.port, srv.host
            ));
        } else {
            out.push_str(&format!("# [{}] {}\n{}\n", group_env, srv.name, srv.host));
        }
    }
    out
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ConnectionMode, ResolvedServer};

    fn make_server(name: &str, host: &str, group: &str, port: u16) -> ResolvedServer {
        ResolvedServer {
            namespace: String::new(),
            group_name: group.to_string(),
            env_name: String::new(),
            name: name.to_string(),
            host: host.to_string(),
            user: "admin".to_string(),
            port,
            ssh_key: String::new(),
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
            tags: vec![],
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
    fn empty_produces_header_only() {
        let out = to_nmap_targets(&[]);
        assert!(out.contains("0 serveur(s)"));
        assert_eq!(out.lines().count(), 1);
    }

    #[test]
    fn standard_port_no_port_annotation() {
        let srv = make_server("web-01", "198.51.100.1", "prod", 22);
        let out = to_nmap_targets(&[&srv]);
        assert!(out.contains("198.51.100.1"));
        assert!(!out.contains("(port"));
    }

    #[test]
    fn non_standard_port_annotated() {
        let srv = make_server("bastion", "198.51.100.2", "infra", 2222);
        let out = to_nmap_targets(&[&srv]);
        assert!(out.contains("(port 2222)"));
        assert!(out.contains("198.51.100.2"));
    }

    #[test]
    fn group_env_in_comment() {
        let mut srv = make_server("api", "198.51.100.3", "mygroup", 22);
        srv.env_name = "production".to_string();
        let out = to_nmap_targets(&[&srv]);
        assert!(out.contains("[mygroup/production]"));
    }
}
