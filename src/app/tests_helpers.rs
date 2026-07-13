use super::*;

/// Builds an `App` from a `Config` with the common empty state used across
/// tests (no favorites, fake state path, no expanded items).
pub(super) fn make_test_app(config: Config) -> App {
    App::new(config, vec![], std::path::PathBuf::from("/fake"), vec![]).unwrap()
}

/// Config with a namespace that contains a group with an environment,
/// useful for testing namespaced-env expansion and favorites.
pub(super) fn make_namespace_env_config() -> Config {
    use crate::config::{Environment, NamespaceEntry, Server};

    Config {
        defaults: None,
        includes: vec![],
        groups: vec![ConfigEntry::Namespace(NamespaceEntry {
            label: "NS1".to_string(),
            source_path: "/fake/ns1.yml".to_string(),
            defaults: None,
            vars: Default::default(),
            entries: vec![ConfigEntry::Group(crate::config::Group {
                name: "GrpA".to_string(),
                user: None,
                ssh_key: None,
                mode: None,
                ssh_port: None,
                ssh_options: None,
                wallix: None,
                wallix_group: None,
                jump: None,
                probe_filesystems: None,
                tunnels: None,
                tags: None,
                environments: Some(vec![Environment {
                    name: "EnvA".to_string(),
                    user: None,
                    ssh_key: None,
                    mode: None,
                    ssh_port: None,
                    ssh_options: None,
                    wallix: None,
                    wallix_group: None,
                    jump: None,
                    probe_filesystems: None,
                    tunnels: None,
                    tags: None,
                    servers: vec![Server {
                        name: "ns_srv".to_string(),
                        host: "198.51.100.10".to_string(),
                        user: None,
                        ssh_key: None,
                        ssh_port: None,
                        ssh_options: None,
                        mode: None,
                        wallix: None,
                        jump: None,
                        probe_filesystems: None,
                        tunnels: None,
                        tags: None,
                        ..Default::default()
                    }],
                }]),
                servers: None,
            })],
        })],
        vars: Default::default(),
    }
}

pub(super) fn make_namespace_config() -> Config {
    use crate::config::{NamespaceEntry, Server};

    Config {
        defaults: None,
        includes: vec![],
        groups: vec![
            ConfigEntry::Group(crate::config::Group {
                name: "RootGroup".to_string(),
                user: None,
                ssh_key: None,
                mode: None,
                ssh_port: None,
                ssh_options: None,
                wallix: None,
                wallix_group: None,
                jump: None,
                probe_filesystems: None,
                environments: None,
                tunnels: None,
                tags: None,
                servers: Some(vec![Server {
                    name: "root_srv".to_string(),
                    host: "203.0.113.1".to_string(),
                    user: None,
                    ssh_key: None,
                    ssh_port: None,
                    ssh_options: None,
                    mode: None,
                    wallix: None,
                    jump: None,
                    probe_filesystems: None,
                    tunnels: None,
                    tags: None,
                    ..Default::default()
                }]),
            }),
            ConfigEntry::Namespace(NamespaceEntry {
                label: "CES".to_string(),
                source_path: "/fake/ces.yml".to_string(),
                defaults: None,
                vars: Default::default(),
                entries: vec![ConfigEntry::Group(crate::config::Group {
                    name: "CES_Group".to_string(),
                    user: None,
                    ssh_key: None,
                    mode: None,
                    ssh_port: None,
                    ssh_options: None,
                    wallix: None,
                    wallix_group: None,
                    jump: None,
                    probe_filesystems: None,
                    environments: None,
                    tunnels: None,
                    tags: None,
                    servers: Some(vec![Server {
                        name: "ces_srv".to_string(),
                        host: "203.0.113.2".to_string(),
                        user: None,
                        ssh_key: None,
                        ssh_port: None,
                        ssh_options: None,
                        mode: None,
                        wallix: None,
                        jump: None,
                        probe_filesystems: None,
                        tunnels: None,
                        tags: None,
                        ..Default::default()
                    }]),
                })],
            }),
        ],
        vars: Default::default(),
    }
}
