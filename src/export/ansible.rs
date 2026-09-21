//! Export d'inventaire Ansible depuis la configuration susshi.
//!
//! # Format de sortie
//!
//! ```yaml
//! all:
//!   children:
//!     <groupe>:
//!       hosts:
//!         <serveur>:
//!           ansible_host: 198.51.100.1
//!           ansible_user: admin
//!           ansible_port: 22
//!           ansible_ssh_private_key_file: ~/.ssh/prod_ed25519
//!       children:
//!         <environnement>:
//!           hosts:
//!             ...
//!     <namespace>:
//!       children:
//!         ...
//! ```
//!
//! - Les **groupes** susshi → groupes Ansible (`children`).
//! - Les **environnements** → sous-groupes.
//! - Les **namespaces** (includes) → groupe de haut niveau.

use crate::config::ResolvedServer;
use std::collections::BTreeMap;

// ─── Filtre ──────────────────────────────────────────────────────────────────

/// Filtre les serveurs selon une requête texte + `#tag`.
///
/// Syntaxe identique à la barre de recherche TUI :
/// - Tokens sans `#` → recherche textuelle sur `name` et `host` (AND).
/// - Tokens avec `#` → filtre de tag exact (AND).
pub fn filter_servers<'a>(servers: &'a [ResolvedServer], query: &str) -> Vec<&'a ResolvedServer> {
    if query.trim().is_empty() {
        return servers.iter().collect();
    }
    let (text_tokens, tag_tokens) = parse_filter_tokens(query);
    servers
        .iter()
        .filter(|s| {
            let name_lc = s.name.to_lowercase();
            let host_lc = s.host.to_lowercase();
            let text_ok = text_tokens
                .iter()
                .all(|t| name_lc.contains(t.as_str()) || host_lc.contains(t.as_str()));
            let tags_ok = tag_tokens
                .iter()
                .all(|t| s.tags.iter().any(|stag| stag.to_lowercase() == t.as_str()));
            text_ok && tags_ok
        })
        .collect()
}

fn parse_filter_tokens(q: &str) -> (Vec<String>, Vec<String>) {
    let mut text = Vec::new();
    let mut tags = Vec::new();
    for word in q.split_whitespace() {
        if let Some(tag) = word.strip_prefix('#') {
            if !tag.is_empty() {
                tags.push(tag.to_lowercase());
            }
        } else {
            text.push(word.to_lowercase());
        }
    }
    (text, tags)
}

// ─── Génération YAML Ansible ─────────────────────────────────────────────────

/// Convertit un identifiant susshi en clé Ansible valide :
/// espaces, `/`, `.` → `_` ; mise en minuscules.
fn ansible_key(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

/// Écrit les variables d'un hôte dans `out` avec le `indent` fourni.
fn write_host_vars(out: &mut String, srv: &ResolvedServer, indent: &str) {
    out.push_str(&format!("{}ansible_host: {}\n", indent, srv.host));
    out.push_str(&format!("{}ansible_user: {}\n", indent, srv.user));
    out.push_str(&format!("{}ansible_port: {}\n", indent, srv.port));
    if !srv.ssh_key.is_empty() {
        out.push_str(&format!(
            "{}ansible_ssh_private_key_file: {}\n",
            indent, srv.ssh_key
        ));
    }
}

/// Écrit un bloc `hosts:` pour une liste de serveurs.
fn write_hosts_block(
    out: &mut String,
    indices: &[usize],
    servers: &[&ResolvedServer],
    indent: &str,
) {
    out.push_str(&format!("{}hosts:\n", indent));
    for &i in indices {
        let srv = servers[i];
        out.push_str(&format!("{}  {}:\n", indent, ansible_key(&srv.name)));
        write_host_vars(out, srv, &format!("{}    ", indent));
    }
}

/// Écrit les groupes contenus dans `group_map` sous l'`indent` courant.
///
/// `group_map` : `group_name` → (`env_name` → indices dans `servers`).
fn write_groups(
    out: &mut String,
    group_map: &BTreeMap<String, BTreeMap<String, Vec<usize>>>,
    servers: &[&ResolvedServer],
    indent: &str,
) {
    for (grp_name, envs) in group_map {
        let grp_key = if grp_name.is_empty() {
            "ungrouped".to_string()
        } else {
            ansible_key(grp_name)
        };
        out.push_str(&format!("{}{}:\n", indent, grp_key));

        // Hôtes directement dans le groupe (sans environnement)
        if let Some(root_indices) = envs.get("") {
            write_hosts_block(out, root_indices, servers, &format!("{}  ", indent));
        }

        // Sous-groupes (environnements)
        let sub_envs: Vec<_> = envs.iter().filter(|(e, _)| !e.is_empty()).collect();
        if !sub_envs.is_empty() {
            out.push_str(&format!("{}  children:\n", indent));
            for (env_name, indices) in &sub_envs {
                out.push_str(&format!("{}    {}:\n", indent, ansible_key(env_name)));
                write_hosts_block(out, indices, servers, &format!("{}      ", indent));
            }
        }
    }
}

/// Génère un inventaire Ansible YAML depuis une liste de serveurs résolus.
///
/// La sortie suit le format YAML Ansible (`all.children`).
pub fn to_ansible_yaml(servers: &[&ResolvedServer]) -> String {
    // Arbre : namespace → groupe → environnement → indices
    let mut tree: BTreeMap<String, BTreeMap<String, BTreeMap<String, Vec<usize>>>> =
        BTreeMap::new();

    for (i, srv) in servers.iter().enumerate() {
        tree.entry(srv.namespace.clone())
            .or_default()
            .entry(srv.group_name.clone())
            .or_default()
            .entry(srv.env_name.clone())
            .or_default()
            .push(i);
    }

    let mut out = String::from("all:\n  children:\n");

    // Serveurs du fichier principal (namespace vide) → directement sous `children`
    if let Some(main_groups) = tree.get("") {
        write_groups(&mut out, main_groups, servers, "    ");
    }

    // Serveurs issus des namespaces (includes) → groupe de haut niveau
    for (ns_name, groups) in &tree {
        if ns_name.is_empty() {
            continue;
        }
        out.push_str(&format!("    {}:\n", ansible_key(ns_name)));
        out.push_str("      children:\n");
        write_groups(&mut out, groups, servers, "        ");
    }

    out
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ConnectionMode, ResolvedServer};

    fn make_server(
        name: &str,
        host: &str,
        group: &str,
        env: &str,
        namespace: &str,
        tags: Vec<&str>,
    ) -> ResolvedServer {
        ResolvedServer {
            namespace: namespace.to_string(),
            group_name: group.to_string(),
            env_name: env.to_string(),
            name: name.to_string(),
            host: host.to_string(),
            user: "admin".to_string(),
            port: 22,
            ssh_key: String::new(),
            ssh_options: vec![],
            default_mode: ConnectionMode::Direct,
            jump_host: None,
            bastion_host: None,
            bastion_user: None,
            bastion_template: String::new(),
            use_system_ssh_config: false,
            probe_filesystems: vec![],
            tunnels: vec![],
            tags: tags.into_iter().map(|t| t.to_string()).collect(),
            control_master: false,
            agent_forwarding: false,
            control_path: String::new(),
            control_persist: "10m".to_string(),
            pre_connect_hook: None,
            post_disconnect_hook: None,
            hook_timeout_secs: 5,
            ssh_cert: String::new(),
            notes: String::new(),
            production: false,
            confirm_production: false,
            ssh_agent_sock: String::new(),
            wallix_group: None,
            wallix_account: "default".to_string(),
            wallix_protocol: "SSH".to_string(),
            wallix_auto_select: true,
            wallix_fail_if_menu_match_error: true,
            wallix_selection_timeout_secs: 8,
            wallix_direct: false,
            wallix_authorization: None,
            wallix_header_columns: vec![],
        }
    }

    #[test]
    fn empty_servers_produces_minimal_yaml() {
        let yaml = to_ansible_yaml(&[]);
        assert_eq!(yaml, "all:\n  children:\n");
    }

    #[test]
    fn single_server_group_no_env() {
        let srv = make_server("web-01", "198.51.100.1", "prod", "", "", vec![]);
        let yaml = to_ansible_yaml(&[&srv]);
        assert!(yaml.contains("prod:"), "group missing");
        assert!(yaml.contains("web-01:"), "server key missing");
        assert!(yaml.contains("ansible_host: 198.51.100.1"));
        assert!(yaml.contains("ansible_user: admin"));
        assert!(yaml.contains("ansible_port: 22"));
    }

    #[test]
    fn server_with_env_creates_children_subgroup() {
        let srv = make_server("api-01", "198.51.100.2", "workers", "prod", "", vec![]);
        let yaml = to_ansible_yaml(&[&srv]);
        assert!(yaml.contains("workers:"));
        assert!(yaml.contains("children:"));
        assert!(yaml.contains("prod:"));
        assert!(yaml.contains("api-01:"));
    }

    #[test]
    fn namespace_creates_top_level_group() {
        let srv = make_server("srv", "203.0.113.1", "grp", "", "CES", vec![]);
        let yaml = to_ansible_yaml(&[&srv]);
        assert!(yaml.contains("ces:"), "namespace group missing");
        assert!(yaml.contains("grp:"));
        assert!(yaml.contains("srv:"));
    }

    #[test]
    fn filter_by_text() {
        let s1 = make_server("web-prod", "198.51.100.1", "", "", "", vec![]);
        let s2 = make_server("db-prod", "198.51.100.2", "", "", "", vec![]);
        let all = vec![s1, s2];
        let filtered = filter_servers(&all, "web");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "web-prod");
    }

    #[test]
    fn filter_by_tag() {
        let s1 = make_server("web", "203.0.113.1", "", "", "", vec!["prod", "web"]);
        let s2 = make_server("db", "203.0.113.2", "", "", "", vec!["staging", "db"]);
        let all = vec![s1, s2];
        let filtered = filter_servers(&all, "#prod");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "web");
    }

    #[test]
    fn filter_empty_returns_all() {
        let s1 = make_server("a", "203.0.113.1", "", "", "", vec![]);
        let s2 = make_server("b", "203.0.113.2", "", "", "", vec![]);
        let all = vec![s1, s2];
        assert_eq!(filter_servers(&all, "").len(), 2);
    }

    #[test]
    fn ansible_key_sanitizes_special_chars() {
        assert_eq!(ansible_key("Prod Web"), "prod_web");
        assert_eq!(ansible_key("eu/west"), "eu_west");
        assert_eq!(ansible_key("v1.0"), "v1_0");
    }

    #[test]
    fn ssh_key_included_when_non_empty() {
        let mut srv = make_server("s", "203.0.113.1", "g", "", "", vec![]);
        srv.ssh_key = "~/.ssh/prod_ed25519".to_string();
        let yaml = to_ansible_yaml(&[&srv]);
        assert!(yaml.contains("ansible_ssh_private_key_file: ~/.ssh/prod_ed25519"));
    }
}
