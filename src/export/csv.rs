//! Export CSV depuis l'inventaire susshi.
//!
//! En-tête : `name,host,user,port,ssh_key,group,env,namespace,tags,notes`
//!
//! Les champs contenant une virgule, un guillemet ou un saut de ligne
//! sont encadrés de guillemets doubles (RFC 4180). Les guillemets internes
//! sont doublés.

use crate::config::ResolvedServer;

fn escape_field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

pub fn to_csv_string(servers: &[&ResolvedServer]) -> String {
    let mut out = String::from("name,host,user,port,ssh_key,group,env,namespace,tags,notes\n");
    for s in servers {
        let tags = s.tags.join(";");
        let row = [
            escape_field(&s.name),
            escape_field(&s.host),
            escape_field(&s.user),
            s.port.to_string(),
            escape_field(&s.ssh_key),
            escape_field(&s.group_name),
            escape_field(&s.env_name),
            escape_field(&s.namespace),
            escape_field(&tags),
            escape_field(&s.notes),
        ]
        .join(",");
        out.push_str(&row);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ConnectionMode, ResolvedServer};

    fn make_server(name: &str, host: &str, group: &str, tags: Vec<&str>) -> ResolvedServer {
        ResolvedServer {
            name: name.into(),
            host: host.into(),
            user: "ops".into(),
            port: 22,
            ssh_key: String::new(),
            group_name: group.into(),
            env_name: String::new(),
            namespace: String::new(),
            tags: tags.into_iter().map(String::from).collect(),
            notes: String::new(),
            production: false,
            confirm_production: false,
            default_mode: ConnectionMode::Direct,
            jump_host: None,
            bastion_host: None,
            bastion_user: None,
            bastion_template: String::new(),
            use_system_ssh_config: false,
            probe_filesystems: vec![],
            tunnels: vec![],
            ssh_options: vec![],
            control_master: false,
            agent_forwarding: false,
            control_path: String::new(),
            control_persist: String::new(),
            pre_connect_hook: None,
            post_disconnect_hook: None,
            hook_timeout_secs: 5,
            ssh_cert: String::new(),
            ssh_agent_sock: String::new(),
            wallix_group: None,
            wallix_account: String::new(),
            wallix_protocol: String::new(),
            wallix_auto_select: false,
            wallix_fail_if_menu_match_error: false,
            wallix_selection_timeout_secs: 8,
            wallix_direct: false,
            wallix_authorization: None,
            wallix_header_columns: vec![],
        }
    }

    #[test]
    fn csv_header_present() {
        let out = to_csv_string(&[]);
        assert!(out.starts_with("name,host,user,port,ssh_key,group,env,namespace,tags,notes\n"));
    }

    #[test]
    fn csv_basic_row() {
        let s = make_server("api-01", "198.51.100.1", "prod", vec!["web", "prod"]);
        let out = to_csv_string(&[&s]);
        // Vérifie les colonnes clés individuellement
        assert!(out.contains("api-01"), "name absent");
        assert!(out.contains("198.51.100.1"), "host absent");
        assert!(out.contains("ops"), "user absent");
        assert!(out.contains(",22,"), "port absent");
        assert!(out.contains("web;prod"), "tags absents");
    }

    #[test]
    fn csv_escapes_commas_in_field() {
        let mut s = make_server("db", "10.0.0.1", "grp", vec![]);
        s.notes = "primary, high-traffic".into();
        let out = to_csv_string(&[&s]);
        assert!(
            out.contains("\"primary, high-traffic\""),
            "virgule doit être échappée"
        );
    }

    #[test]
    fn csv_escapes_quotes_in_field() {
        let mut s = make_server("db", "10.0.0.1", "grp", vec![]);
        s.notes = "say \"hello\"".into();
        let out = to_csv_string(&[&s]);
        assert!(
            out.contains("\"say \"\"hello\"\"\""),
            "guillemets doublés attendus"
        );
    }
}
