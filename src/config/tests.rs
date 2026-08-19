use super::*;

// ─── Tests interpolate / undefined_vars ───────────────────────────────────

#[test]
fn test_interpolate_replaces_known_vars() {
    let vars = HashMap::from([
        ("host".to_string(), "bastion.prod.example.com".to_string()),
        ("env".to_string(), "prod".to_string()),
    ]);
    assert_eq!(interpolate("{{ host }}", &vars), "bastion.prod.example.com");
    assert_eq!(interpolate("{{ env }}-server", &vars), "prod-server");
    assert_eq!(
        interpolate("{{ env }}.{{ host }}", &vars),
        "prod.bastion.prod.example.com"
    );
}

#[test]
fn test_interpolate_leaves_undefined_vars() {
    let vars = HashMap::new();
    assert_eq!(interpolate("{{ unknown }}", &vars), "{{ unknown }}");
}

#[test]
fn test_interpolate_no_placeholder() {
    let vars = HashMap::from([("x".to_string(), "y".to_string())]);
    assert_eq!(interpolate("plain-host", &vars), "plain-host");
}

#[test]
fn test_undefined_vars_finds_missing() {
    let vars = HashMap::from([("a".to_string(), "1".to_string())]);
    let result = undefined_vars("{{ a }} and {{ b }}", &vars);
    assert_eq!(result, vec!["b".to_string()]);
}

#[test]
fn test_undefined_vars_empty_when_all_defined() {
    let vars = HashMap::from([("x".to_string(), "v".to_string())]);
    assert!(undefined_vars("{{ x }}", &vars).is_empty());
}

#[test]
fn test_resolve_applies_interpolation() {
    let vars = HashMap::from([("jump".to_string(), "bastion.example.com".to_string())]);
    let config = Config {
        defaults: None,
        groups: vec![ConfigEntry::Group(Group {
            name: "G".to_string(),
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
                name: "jump-srv".to_string(),
                host: "{{ jump }}".to_string(),
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
        includes: vec![],
        vars,
    };

    let resolved = config.resolve().unwrap();
    assert_eq!(resolved[0].host, "bastion.example.com");
    assert_eq!(resolved[0].name, "jump-srv");
}

#[test]
fn test_merge_bastion() {
    let parent = Some(BastionConfig {
        host: Some("parent_host".to_string()),
        user: Some("parent_user".to_string()),
        group: Some("parent-group".to_string()),
        template: Some("parent_tmpl".to_string()),
        account: None,
        protocol: None,
        auto_select: None,
        fail_if_menu_match_error: None,
        selection_timeout_secs: None,
        direct: None,
        authorization: None,
        header_columns: None,
    });
    let child = BastionConfig {
        host: None,
        user: Some("child_user".to_string()),
        group: None,
        template: None,
        account: None,
        protocol: None,
        auto_select: None,
        fail_if_menu_match_error: None,
        selection_timeout_secs: None,
        direct: None,
        authorization: None,
        header_columns: None,
    };

    let merged = merge_bastion(&parent, &Some(child)).unwrap();
    // Child user overrides parent
    assert_eq!(merged.user, Some("child_user".to_string()));
    // Parent host is inherited
    assert_eq!(merged.host, Some("parent_host".to_string()));
    // Parent template is inherited
    assert_eq!(merged.template, Some("parent_tmpl".to_string()));
    // Parent group is inherited
    assert_eq!(merged.group, Some("parent-group".to_string()));
}

#[test]
fn test_resolve_inherits_wallix_group_from_defaults_wallix() {
    let config = Config {
        defaults: Some(Defaults {
            mode: Some(ConnectionMode::Wallix),
            wallix: Some(BastionConfig {
                host: Some("bastion.example.test".to_string()),
                user: Some("demo_user".to_string()),
                group: Some("dev-admins".to_string()),
                template: None,
                account: None,
                protocol: None,
                auto_select: None,
                fail_if_menu_match_error: None,
                selection_timeout_secs: None,
                direct: None,
                authorization: None,
                header_columns: None,
            }),
            ..Default::default()
        }),
        groups: vec![ConfigEntry::Group(Group {
            name: "ALPHA-BD".to_string(),
            user: None,
            ssh_key: None,
            mode: None,
            ssh_port: None,
            ssh_options: None,
            wallix: None,
            wallix_group: None,
            jump: None,
            environments: None,
            servers: Some(vec![Server {
                name: "app-alpha".to_string(),
                host: "APP-ALPHA-BD".to_string(),
                user: None,
                ssh_key: None,
                ssh_cert: None,
                ssh_port: None,
                ssh_options: None,
                mode: None,
                wallix: None,
                wallix_group: None,
                jump: None,
                probe_filesystems: None,
                tunnels: None,
                tags: None,
                pre_connect_hook: None,
                post_disconnect_hook: None,
                notes: None,
                ssh_agent_sock: None,
            }]),
            probe_filesystems: None,
            tunnels: None,
            tags: None,
        })],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    assert_eq!(resolved[0].wallix_group.as_deref(), Some("dev-admins"));
}

#[test]
fn test_resolve_wallix_group_server_override_wins_over_global() {
    let config = Config {
        defaults: Some(Defaults {
            wallix: Some(BastionConfig {
                group: Some("global-admins".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        }),
        groups: vec![ConfigEntry::Server(Server {
            name: "srv".to_string(),
            host: "srv.example.test".to_string(),
            wallix: Some(BastionConfig {
                group: Some("conn-admins".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        })],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    assert_eq!(resolved[0].wallix_group.as_deref(), Some("conn-admins"));
}

#[test]
fn test_resolve_wallix_group_env_override_wins_over_global() {
    let config = Config {
        defaults: Some(Defaults {
            wallix: Some(BastionConfig {
                group: Some("global-admins".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        }),
        groups: vec![ConfigEntry::Group(Group {
            name: "G".to_string(),
            user: None,
            ssh_key: None,
            mode: None,
            ssh_port: None,
            ssh_options: None,
            wallix: None,
            wallix_group: None,
            jump: None,
            environments: Some(vec![Environment {
                name: "PR-OND".to_string(),
                user: None,
                ssh_key: None,
                mode: None,
                ssh_port: None,
                ssh_options: None,
                wallix: Some(BastionConfig {
                    group: Some("env-admins".to_string()),
                    ..Default::default()
                }),
                wallix_group: None,
                jump: None,
                servers: vec![Server {
                    name: "db07".to_string(),
                    host: "db07.internal.example".to_string(),
                    ..Default::default()
                }],
                probe_filesystems: None,
                tunnels: None,
                tags: None,
            }]),
            servers: None,
            probe_filesystems: None,
            tunnels: None,
            tags: None,
        })],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    assert_eq!(resolved[0].wallix_group.as_deref(), Some("env-admins"));
}

#[test]
fn test_resolve_wallix_group_none_when_missing_everywhere() {
    let config = Config {
        defaults: Some(Defaults::default()),
        groups: vec![ConfigEntry::Server(Server {
            name: "srv".to_string(),
            host: "srv.example.test".to_string(),
            ..Default::default()
        })],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    assert_eq!(resolved[0].wallix_group, None);
}

#[test]
fn test_sorting_mixed() {
    let mut config = Config {
        defaults: None,
        groups: vec![
            ConfigEntry::Group(Group {
                name: "Zeus".to_string(),
                user: None,
                ssh_key: None,
                mode: None,
                ssh_port: None,
                ssh_options: None,
                wallix: None,
                wallix_group: None,
                jump: None,
                environments: None,
                servers: None,
                probe_filesystems: None,
                tunnels: None,
                tags: None,
            }),
            ConfigEntry::Server(Server {
                name: "Alpha".to_string(),
                host: "198.51.100.1".to_string(),
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
            }),
            ConfigEntry::Group(Group {
                name: "Beta".to_string(),
                user: None,
                ssh_key: None,
                mode: None,
                ssh_port: None,
                ssh_options: None,
                wallix: None,
                wallix_group: None,
                jump: None,
                environments: None,
                servers: None,
                probe_filesystems: None,
                tunnels: None,
                tags: None,
            }),
        ],
        includes: vec![],
        vars: Default::default(),
    };

    config.sort();

    // Check order: Alpha, Beta, Zeus
    match &config.groups[0] {
        ConfigEntry::Server(s) => assert_eq!(s.name, "Alpha"),
        _ => panic!("Expected Alpha first"),
    }
    match &config.groups[1] {
        ConfigEntry::Group(g) => assert_eq!(g.name, "Beta"),
        _ => panic!("Expected Beta second"),
    }
    match &config.groups[2] {
        ConfigEntry::Group(g) => assert_eq!(g.name, "Zeus"),
        _ => panic!("Expected Zeus third"),
    }
}

#[test]
fn test_resolve_inheritance_chain() {
    let config = Config {
        defaults: Some(Defaults {
            user: Some("default_user".to_string()),
            ssh_port: Some(2222),
            ..Default::default()
        }),
        groups: vec![ConfigEntry::Group(Group {
            name: "G1".to_string(),
            user: Some("group_user".to_string()), // Override default
            ssh_key: None,
            mode: None,
            ssh_port: None, // Inherits 2222
            ssh_options: None,
            wallix: None,
            wallix_group: None,
            jump: None,
            probe_filesystems: None,
            tags: None,
            environments: Some(vec![Environment {
                name: "Env1".to_string(),
                user: None, // Inherits "group_user"
                ssh_key: None,
                mode: None,
                ssh_port: None, // Inherits 2222
                ssh_options: None,
                wallix: None,
                wallix_group: None,
                jump: None,
                probe_filesystems: None,
                tunnels: None,
                tags: None,
                servers: vec![Server {
                    name: "S1".to_string(),
                    host: "203.0.113.1".to_string(),
                    user: None, // Inherits "group_user"
                    ssh_key: None,
                    ssh_port: Some(8080), // Override 2222
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
            tunnels: None,
        })],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    let s1 = &resolved[0];

    assert_eq!(s1.name, "S1");
    assert_eq!(s1.user, "group_user");
    assert_eq!(s1.port, 8080);
}

#[test]
fn test_probe_filesystems_inheritance() {
    let config = Config {
        defaults: Some(Defaults {
            probe_filesystems: Some(vec!["/data".to_string(), "/var/log".to_string()]),
            ..Default::default()
        }),
        groups: vec![ConfigEntry::Group(Group {
            name: "G".to_string(),
            user: None,
            ssh_key: None,
            mode: None,
            ssh_port: None,
            ssh_options: None,
            wallix: None,
            wallix_group: None,
            jump: None,
            probe_filesystems: None, // hérite des defaults
            environments: None,
            tunnels: None,
            tags: None,
            servers: Some(vec![
                Server {
                    name: "inherits".to_string(),
                    host: "203.0.113.4".to_string(),
                    user: None,
                    ssh_key: None,
                    ssh_port: None,
                    ssh_options: None,
                    mode: None,
                    wallix: None,
                    jump: None,
                    probe_filesystems: None, // hérite du groupe → defaults
                    tunnels: None,
                    tags: None,
                    ..Default::default()
                },
                Server {
                    name: "extends".to_string(),
                    host: "203.0.113.5".to_string(),
                    user: None,
                    ssh_key: None,
                    ssh_port: None,
                    ssh_options: None,
                    mode: None,
                    wallix: None,
                    jump: None,
                    probe_filesystems: Some(vec!["/mnt/nas".to_string()]), // s'ajoute aux defaults
                    tunnels: None,
                    tags: None,
                    ..Default::default()
                },
            ]),
        })],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();

    let inherits = resolved.iter().find(|s| s.name == "inherits").unwrap();
    assert_eq!(
        inherits.probe_filesystems,
        vec!["/data".to_string(), "/var/log".to_string()]
    );

    // Le serveur ajoute /mnt/nas aux defaults — il ne les remplace PAS
    let extends = resolved.iter().find(|s| s.name == "extends").unwrap();
    assert_eq!(
        extends.probe_filesystems,
        vec![
            "/data".to_string(),
            "/var/log".to_string(),
            "/mnt/nas".to_string()
        ]
    );
}

#[test]
fn test_probe_filesystems_group_extends_defaults() {
    let config = Config {
        defaults: Some(Defaults {
            probe_filesystems: Some(vec!["/pg_backup".to_string(), "/pg_xlogs".to_string()]),
            ..Default::default()
        }),
        groups: vec![ConfigEntry::Group(Group {
            name: "ALPHA".to_string(),
            user: None,
            ssh_key: None,
            mode: None,
            ssh_port: None,
            ssh_options: None,
            wallix: None,
            wallix_group: None,
            jump: None,
            probe_filesystems: Some(vec!["/kafka_data".to_string()]), // s'ajoute
            environments: None,
            tunnels: None,
            tags: None,
            servers: Some(vec![Server {
                name: "kafka01".to_string(),
                host: "198.51.100.1".to_string(),
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
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    let kafka = resolved.iter().find(|s| s.name == "kafka01").unwrap();

    // Le groupe ajoute /kafka_data aux defaults — PG filesystems toujours présents
    assert_eq!(
        kafka.probe_filesystems,
        vec![
            "/pg_backup".to_string(),
            "/pg_xlogs".to_string(),
            "/kafka_data".to_string()
        ]
    );
}

// ─── Tests includes / namespaces ─────────────────────────────────────────

fn write_temp_yaml(content: &str) -> tempfile::NamedTempFile {
    use std::io::Write;
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(content.as_bytes()).unwrap();
    f
}

#[test]
fn test_includes_basic() {
    let sub_yaml = r#"
defaults:
  user: "sub_user"
groups:
  - name: NS_Group
    servers:
      - name: ns_srv
        host: "198.51.100.1"
"#;
    let sub_file = write_temp_yaml(sub_yaml);

    let main_yaml = format!(
        r#"
defaults:
  user: "main_user"
includes:
  - label: "CES"
    path: "{}"
groups:
  - name: Main_Group
    servers:
      - name: main_srv
        host: "198.51.100.1"
"#,
        sub_file.path().to_string_lossy()
    );
    let main_file = write_temp_yaml(&main_yaml);

    let (config, warnings, _val) =
        Config::load_merged(main_file.path(), &mut std::collections::HashSet::new(), 0).unwrap();
    assert!(warnings.is_empty(), "Expected no warnings: {:?}", warnings);

    let resolved = config.resolve().unwrap();

    // main_srv has empty namespace
    let main_srv = resolved.iter().find(|s| s.name == "main_srv").unwrap();
    assert_eq!(main_srv.namespace, "");
    assert_eq!(main_srv.user, "main_user");

    // ns_srv has namespace "CES" and uses sub-config defaults
    let ns_srv = resolved.iter().find(|s| s.name == "ns_srv").unwrap();
    assert_eq!(ns_srv.namespace, "CES");
    assert_eq!(ns_srv.user, "sub_user");

    // Config tree should contain a Namespace entry
    let has_namespace = config.groups.iter().any(|e| {
        if let ConfigEntry::Namespace(ns) = e {
            ns.label == "CES"
        } else {
            false
        }
    });
    assert!(has_namespace, "Expected Namespace(CES) in config.groups");
}

#[test]
fn test_includes_duplicate_label_merges_into_single_namespace() {
    // Deux fichiers inclus séparément sous le même label doivent produire
    // un seul nœud Namespace dans l'arbre, avec les entrées des deux fusionnées.
    let sub_a_yaml = r#"
groups:
  - name: Group_A
    servers:
      - name: srv_a
        host: "198.51.100.1"
"#;
    let sub_b_yaml = r#"
groups:
  - name: Group_B
    servers:
      - name: srv_b
        host: "198.51.100.2"
"#;
    let sub_a_file = write_temp_yaml(sub_a_yaml);
    let sub_b_file = write_temp_yaml(sub_b_yaml);

    let main_yaml = format!(
        r#"
includes:
  - label: "CES 3S"
    path: "{}"
  - label: "CES 3S"
    path: "{}"
"#,
        sub_a_file.path().to_string_lossy(),
        sub_b_file.path().to_string_lossy()
    );
    let main_file = write_temp_yaml(&main_yaml);

    let (config, warnings, _val) =
        Config::load_merged(main_file.path(), &mut std::collections::HashSet::new(), 0).unwrap();
    assert!(warnings.is_empty(), "Expected no warnings: {:?}", warnings);

    let namespaces: Vec<_> = config
        .groups
        .iter()
        .filter_map(|e| match e {
            ConfigEntry::Namespace(ns) if ns.label == "CES 3S" => Some(ns),
            _ => None,
        })
        .collect();
    assert_eq!(
        namespaces.len(),
        1,
        "Expected a single merged Namespace(CES 3S), got {}",
        namespaces.len()
    );
    assert_eq!(namespaces[0].entries.len(), 2);

    let resolved = config.resolve().unwrap();
    assert!(resolved.iter().any(|s| s.name == "srv_a"));
    assert!(resolved.iter().any(|s| s.name == "srv_b"));
}

#[test]
fn test_includes_only_file_omits_empty_umbrella_namespace() {
    // Un fichier racine qui ne fait que `!include` d'autres fichiers, sans
    // `groups:` explicite ni entrée directe, ne doit produire ni erreur de
    // parsing ni nœud "CES 3S" vide dans l'arbre : seuls les sous-namespaces
    // "CES 3S / Colibris" et "CES 3S / Scolarité" doivent apparaître.
    let colibris_yaml = r#"
groups:
  - name: COLIBRIS
    servers:
      - name: colibris_srv
        host: "198.51.100.10"
"#;
    let scolarite_yaml = r#"
groups:
  - name: SCOLARITE
    servers:
      - name: scolarite_srv
        host: "198.51.100.11"
"#;
    let colibris_file = write_temp_yaml(colibris_yaml);
    let scolarite_file = write_temp_yaml(scolarite_yaml);

    let umbrella_yaml = format!(
        r#"
includes:
  - label: "Colibris"
    path: "{}"
  - label: "Scolarité"
    path: "{}"
"#,
        colibris_file.path().to_string_lossy(),
        scolarite_file.path().to_string_lossy()
    );
    let umbrella_file = write_temp_yaml(&umbrella_yaml);

    let main_yaml = format!(
        r#"
includes:
  - label: "CES 3S"
    path: "{}"
"#,
        umbrella_file.path().to_string_lossy()
    );
    let main_file = write_temp_yaml(&main_yaml);

    let (config, warnings, _val) =
        Config::load_merged(main_file.path(), &mut std::collections::HashSet::new(), 0).unwrap();
    assert!(warnings.is_empty(), "Expected no warnings: {:?}", warnings);

    let labels: Vec<&str> = config
        .groups
        .iter()
        .filter_map(|e| match e {
            ConfigEntry::Namespace(ns) => Some(ns.label.as_str()),
            _ => None,
        })
        .collect();

    assert!(
        !labels.contains(&"CES 3S"),
        "Empty umbrella namespace 'CES 3S' should not appear in tree, got {:?}",
        labels
    );
    assert!(labels.contains(&"CES 3S / Colibris"));
    assert!(labels.contains(&"CES 3S / Scolarité"));
}

#[test]
fn test_includes_defaults_isolation() {
    let sub_yaml = r#"
defaults:
  user: "sub_user"
  ssh_port: 9999
groups:
  - name: Sub
    servers:
      - name: sub_srv
        host: "203.0.113.4"
"#;
    let sub_file = write_temp_yaml(sub_yaml);

    let main_yaml = format!(
        r#"
defaults:
  user: "main_user"
  ssh_port: 22
includes:
  - label: "SUB"
    path: "{}"
groups:
  - name: Main
    servers:
      - name: main_srv
        host: "203.0.113.8"
"#,
        sub_file.path().to_string_lossy()
    );
    let main_file = write_temp_yaml(&main_yaml);

    let (config, warnings, _val) =
        Config::load_merged(main_file.path(), &mut std::collections::HashSet::new(), 0).unwrap();
    assert!(warnings.is_empty());

    let resolved = config.resolve().unwrap();

    let main_srv = resolved.iter().find(|s| s.name == "main_srv").unwrap();
    // Main defaults apply to main_srv
    assert_eq!(main_srv.user, "main_user");
    assert_eq!(main_srv.port, 22);

    let sub_srv = resolved.iter().find(|s| s.name == "sub_srv").unwrap();
    // Sub defaults apply only to sub_srv, not leaked from main
    assert_eq!(sub_srv.user, "sub_user");
    assert_eq!(sub_srv.port, 9999);
}

#[test]
fn test_includes_missing_file() {
    let main_yaml = r#"
defaults:
  user: "admin"
includes:
  - label: "MISSING"
    path: "/tmp/susshi_nonexistent_test_file_xyz.yml"
groups:
  - name: Main
    servers:
      - name: ok_srv
        host: "203.0.113.4"
"#;
    let main_file = write_temp_yaml(main_yaml);

    let (config, warnings, _val) =
        Config::load_merged(main_file.path(), &mut std::collections::HashSet::new(), 0).unwrap();

    // Un avertissement LoadError doit être émis
    assert_eq!(warnings.len(), 1);
    if let IncludeWarning::LoadError { label, .. } = &warnings[0] {
        assert_eq!(label, "MISSING");
    } else {
        panic!("Expected LoadError warning, got {:?}", warnings[0]);
    }

    // Les groupes du fichier principal sont toujours résolus
    let resolved = config.resolve().unwrap();
    assert!(resolved.iter().any(|s| s.name == "ok_srv"));
}

#[test]
fn test_includes_nested_recursive() {
    // Fichier inclus qui contient lui-même un `includes:` — résolution récursive v0.8
    let leaf_yaml = r#"
groups:
  - name: Leaf
    servers:
      - name: leaf_srv
        host: "203.0.113.9"
"#;
    let leaf_file = write_temp_yaml(leaf_yaml);

    let sub_yaml = format!(
        r#"
includes:
  - label: "LEAF"
    path: "{}"
groups:
  - name: Sub
    servers:
      - name: sub_srv
        host: "203.0.113.18"
"#,
        leaf_file.path().to_string_lossy()
    );
    let sub_file = write_temp_yaml(&sub_yaml);

    let main_yaml = format!(
        r#"
includes:
  - label: "SUB"
    path: "{}"
groups: []
"#,
        sub_file.path().to_string_lossy()
    );
    let main_file = write_temp_yaml(&main_yaml);

    let (config, warnings, _val) =
        Config::load_merged(main_file.path(), &mut std::collections::HashSet::new(), 0).unwrap();

    // Aucun avertissement : les includes imbriqués sont désormais résolus récursivement
    assert!(
        warnings.is_empty(),
        "Expected no warnings, got: {:?}",
        warnings
    );

    // Les deux namespaces aplatis sont présents
    let labels: Vec<&str> = config
        .groups
        .iter()
        .filter_map(|e| {
            if let ConfigEntry::Namespace(ns) = e {
                Some(ns.label.as_str())
            } else {
                None
            }
        })
        .collect();
    assert!(labels.contains(&"SUB"), "Missing SUB, got {:?}", labels);
    assert!(
        labels.contains(&"SUB / LEAF"),
        "Missing 'SUB / LEAF', got {:?}",
        labels
    );

    let resolved = config.resolve().unwrap();
    assert!(
        resolved
            .iter()
            .any(|s| s.name == "sub_srv" && s.namespace == "SUB")
    );
    assert!(
        resolved
            .iter()
            .any(|s| s.name == "leaf_srv" && s.namespace == "SUB / LEAF")
    );
}

#[test]
fn test_includes_merge_defaults() {
    let sub_yaml = r#"
defaults:
  user: "sub_user"
groups:
  - name: Sub
    servers:
      - name: sub_srv
        host: "203.0.113.4"
"#;
    let sub_file = write_temp_yaml(sub_yaml);

    let main_yaml = format!(
        r#"
defaults:
  user: "main_user"
  ssh_port: 2222
includes:
  - label: "SUB"
    path: "{}"
    merge_defaults: true
groups: []
"#,
        sub_file.path().to_string_lossy()
    );
    let main_file = write_temp_yaml(&main_yaml);

    let (config, _warnings, _val) =
        Config::load_merged(main_file.path(), &mut std::collections::HashSet::new(), 0).unwrap();
    let resolved = config.resolve().unwrap();

    let sub_srv = resolved.iter().find(|s| s.name == "sub_srv").unwrap();
    // Sub defaults override main defaults for user
    assert_eq!(sub_srv.user, "sub_user");
    // Main port is inherited since sub didn't specify ssh_port
    assert_eq!(sub_srv.port, 2222);
}

/// Les defaults du fichier principal sont automatiquement hérités par les
/// namespaces inclus, même sans `merge_defaults: true`.
#[test]
fn test_includes_inherit_main_defaults_automatically() {
    let sub_yaml = r#"
groups:
  - name: SubGroup
    servers:
      - name: sub_srv
        host: "2.3.4.5"
"#;
    let sub_file = write_temp_yaml(sub_yaml);

    let main_yaml = format!(
        r#"
defaults:
  user: "main_user"
  ssh_port: 2222
  jump:
    - host: "jump.example.com"
      user: "juser"
includes:
  - label: "SUB"
    path: "{}"
groups: []
"#,
        sub_file.path().to_string_lossy()
    );
    let main_file = write_temp_yaml(&main_yaml);

    let (config, _warnings, _val) =
        Config::load_merged(main_file.path(), &mut std::collections::HashSet::new(), 0).unwrap();
    let resolved = config.resolve().unwrap();

    let sub_srv = resolved.iter().find(|s| s.name == "sub_srv").unwrap();
    // Les defaults du principal doivent être hérités sans merge_defaults: true
    assert_eq!(sub_srv.user, "main_user");
    assert_eq!(sub_srv.port, 2222);
    assert_eq!(sub_srv.jump_host.as_deref(), Some("juser@jump.example.com"));
}

#[test]
fn test_includes_circular() {
    let file_a = tempfile::NamedTempFile::new().unwrap();
    let file_b = tempfile::NamedTempFile::new().unwrap();

    let yaml_a = format!(
        r#"
includes:
  - label: "B"
    path: "{}"
groups:
  - name: GroupA
    servers: [{{ name: srv_a, host: "198.51.100.1" }}]
"#,
        file_b.path().display()
    );
    let yaml_b = format!(
        r#"
includes:
  - label: "A"
    path: "{}"
groups:
  - name: GroupB
    servers: [{{ name: srv_b, host: "198.51.100.2" }}]
"#,
        file_a.path().display()
    );
    std::fs::write(file_a.path(), yaml_a.as_bytes()).unwrap();
    std::fs::write(file_b.path(), yaml_b.as_bytes()).unwrap();

    let (config, warnings, _val) =
        Config::load_merged(file_a.path(), &mut std::collections::HashSet::new(), 0).unwrap();

    let has_circular = warnings
        .iter()
        .any(|w| matches!(w, IncludeWarning::Circular { .. }));
    assert!(
        has_circular,
        "Expected Circular warning, got: {:?}",
        warnings
    );

    let resolved = config.resolve().unwrap();
    assert!(
        resolved
            .iter()
            .any(|s| s.name == "srv_a" || s.name == "srv_b"),
        "Should resolve at least one server"
    );
}

#[test]
fn test_validation_unknown_field() {
    let yaml = r#"
defaults:
  user: "admin"
  typo_field: "oops"
groups: []
"#;
    let warnings = validate_yaml(yaml, "test.yml");
    assert!(
        warnings.iter().any(|w| w.field == "typo_field"),
        "Expected ValidationWarning for typo_field, got: {:?}",
        warnings
    );
}

#[test]
fn test_validation_unknown_server_field() {
    let yaml = r#"
groups:
  - name: G
    servers:
      - name: srv
        host: "203.0.113.4"
        missspelled_user: "admin"
"#;
    let warnings = validate_yaml(yaml, "test.yml");
    assert!(
        warnings.iter().any(|w| w.field == "missspelled_user"),
        "Expected ValidationWarning for missspelled_user, got: {:?}",
        warnings
    );
}

#[test]
fn test_namespace_server_has_namespace_field() {
    let sub_yaml = r#"
groups:
  - name: NS_G
    servers:
      - name: ns_srv
        host: "198.51.100.101"
        user: "ns_user"
"#;
    let sub_file = write_temp_yaml(sub_yaml);

    let main_yaml = format!(
        r#"
includes:
  - label: "CRT"
    path: "{}"
groups: []
"#,
        sub_file.path().to_string_lossy()
    );
    let main_file = write_temp_yaml(&main_yaml);

    let (config, _, _) =
        Config::load_merged(main_file.path(), &mut std::collections::HashSet::new(), 0).unwrap();
    let resolved = config.resolve().unwrap();

    let ns_srv = resolved.iter().find(|s| s.name == "ns_srv").unwrap();
    assert_eq!(ns_srv.namespace, "CRT");
    assert_eq!(ns_srv.group_name, "NS_G");
}

// ─── Tests keep_open ─────────────────────────────────────────────────────

#[test]
fn test_keep_open_absent_defaults_to_none() {
    let yaml = r#"
groups: []
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert!(config.defaults.is_none() || config.defaults.unwrap().keep_open.is_none());
}

#[test]
fn test_keep_open_true_parsed() {
    let yaml = r#"
defaults:
  keep_open: true
groups: []
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    let keep_open = config
        .defaults
        .as_ref()
        .and_then(|d| d.keep_open)
        .unwrap_or(false);
    assert!(keep_open);
}

#[test]
fn test_keep_open_false_parsed() {
    let yaml = r#"
defaults:
  keep_open: false
groups: []
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    let keep_open = config
        .defaults
        .as_ref()
        .and_then(|d| d.keep_open)
        .unwrap_or(true); // on passe true pour détecter si false est bien parsé
    assert!(!keep_open);
}

#[test]
fn test_keep_open_no_validation_warning() {
    let yaml = r#"
defaults:
  keep_open: true
groups: []
"#;
    let warnings = validate_yaml(yaml, "test.yaml");
    assert!(
        warnings.is_empty(),
        "keep_open should not produce a ValidationWarning, got: {:?}",
        warnings
    );
}

// ─── Tests tag inheritance ────────────────────────────────────────────────

fn make_server(name: &str, host: &str, tags: Option<Vec<&str>>) -> Server {
    Server {
        name: name.to_string(),
        host: host.to_string(),
        tags: tags.map(|v| v.into_iter().map(str::to_owned).collect()),
        ..Default::default()
    }
}

fn make_group(name: &str, servers: Vec<Server>) -> Group {
    Group {
        name: name.to_string(),
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
        environments: None,
        servers: Some(servers),
    }
}

fn make_group_with_env(name: &str, envs: Vec<Environment>) -> Group {
    Group {
        name: name.to_string(),
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
        environments: Some(envs),
        servers: None,
    }
}

fn make_env(name: &str, servers: Vec<Server>) -> Environment {
    Environment {
        name: name.to_string(),
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
        servers,
    }
}

#[test]
fn test_tags_union_across_all_levels() {
    let mut env = make_env(
        "E",
        vec![make_server("srv", "10.0.0.1", Some(vec!["srv-tag"]))],
    );
    env.tags = Some(vec!["env-tag".to_string()]);
    let mut group = make_group_with_env("G", vec![env]);
    group.tags = Some(vec!["group-tag".to_string()]);
    let config = Config {
        defaults: Some(Defaults {
            tags: Some(vec!["global".to_string()]),
            ..Default::default()
        }),
        groups: vec![ConfigEntry::Group(group)],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    let tags = &resolved[0].tags;
    assert!(tags.contains(&"global".to_string()), "missing defaults tag");
    assert!(tags.contains(&"group-tag".to_string()), "missing group tag");
    assert!(tags.contains(&"env-tag".to_string()), "missing env tag");
    assert!(tags.contains(&"srv-tag".to_string()), "missing server tag");
    assert_eq!(tags.len(), 4);
}

#[test]
fn test_tags_deduplication_across_levels() {
    let env = make_env(
        "E",
        vec![make_server("srv", "10.0.0.1", Some(vec!["shared"]))],
    );
    let mut group = make_group_with_env("G", vec![env]);
    group.tags = Some(vec!["shared".to_string()]);
    let config = Config {
        defaults: Some(Defaults {
            tags: Some(vec!["shared".to_string(), "global".to_string()]),
            ..Default::default()
        }),
        groups: vec![ConfigEntry::Group(group)],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    let tags = &resolved[0].tags;
    assert_eq!(
        tags.iter().filter(|t| t.as_str() == "shared").count(),
        1,
        "shared tag should appear only once, got: {:?}",
        tags
    );
    assert_eq!(tags.len(), 2);
}

#[test]
fn test_tags_group_level_server_inherits_defaults_and_group() {
    let mut group = make_group("G", vec![make_server("srv", "10.0.0.1", None)]);
    group.tags = Some(vec!["group-tag".to_string()]);
    let config = Config {
        defaults: Some(Defaults {
            tags: Some(vec!["defaults-tag".to_string()]),
            ..Default::default()
        }),
        groups: vec![ConfigEntry::Group(group)],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    let tags = &resolved[0].tags;
    assert!(tags.contains(&"defaults-tag".to_string()));
    assert!(tags.contains(&"group-tag".to_string()));
    assert_eq!(tags.len(), 2);
}

#[test]
fn test_tags_top_level_server_inherits_defaults() {
    let config = Config {
        defaults: Some(Defaults {
            tags: Some(vec!["top-tag".to_string()]),
            ..Default::default()
        }),
        groups: vec![ConfigEntry::Server(Server {
            name: "bare-srv".to_string(),
            host: "10.0.0.1".to_string(),
            ..Default::default()
        })],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    assert!(resolved[0].tags.contains(&"top-tag".to_string()));
}

// ─── Tests tunnel REPLACE semantics ──────────────────────────────────────

fn tunnel(local: u16, remote: u16) -> TunnelConfig {
    TunnelConfig {
        local_port: local,
        remote_host: "127.0.0.1".to_string(),
        remote_port: remote,
        label: String::new(),
    }
}

#[test]
fn test_tunnels_defaults_inherited_when_no_child_defines_them() {
    let config = Config {
        defaults: Some(Defaults {
            tunnels: Some(vec![tunnel(5432, 5432)]),
            ..Default::default()
        }),
        groups: vec![ConfigEntry::Group(make_group_with_env(
            "G",
            vec![make_env("E", vec![make_server("srv", "10.0.0.1", None)])],
        ))],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    assert_eq!(resolved[0].tunnels, vec![tunnel(5432, 5432)]);
}

#[test]
fn test_tunnels_group_replaces_defaults() {
    let mut group = make_group_with_env(
        "G",
        vec![make_env("E", vec![make_server("srv", "10.0.0.1", None)])],
    );
    group.tunnels = Some(vec![tunnel(6379, 6379)]);
    let config = Config {
        defaults: Some(Defaults {
            tunnels: Some(vec![tunnel(5432, 5432)]),
            ..Default::default()
        }),
        groups: vec![ConfigEntry::Group(group)],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    assert_eq!(
        resolved[0].tunnels,
        vec![tunnel(6379, 6379)],
        "group tunnels should fully replace defaults tunnels"
    );
}

#[test]
fn test_tunnels_server_replaces_env() {
    let mut env = make_env(
        "E",
        vec![Server {
            name: "srv".to_string(),
            host: "10.0.0.1".to_string(),
            tunnels: Some(vec![tunnel(8080, 80)]),
            ..Default::default()
        }],
    );
    env.tunnels = Some(vec![tunnel(5432, 5432)]);
    let config = Config {
        defaults: None,
        groups: vec![ConfigEntry::Group(make_group_with_env("G", vec![env]))],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    assert_eq!(
        resolved[0].tunnels,
        vec![tunnel(8080, 80)],
        "server tunnels should fully replace env tunnels"
    );
}

// ─── Tests group-level servers (no environment) ───────────────────────────

#[test]
fn test_group_server_without_environment() {
    let config = Config {
        defaults: Some(Defaults {
            user: Some("default-user".to_string()),
            ssh_port: Some(2222),
            ..Default::default()
        }),
        groups: vec![ConfigEntry::Group(make_group(
            "Flat",
            vec![make_server("srv", "10.0.0.1", None)],
        ))],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    assert_eq!(resolved[0].user, "default-user");
    assert_eq!(resolved[0].port, 2222);
    assert_eq!(resolved[0].group_name, "Flat");
    assert_eq!(resolved[0].env_name, "");
}

// ─── Tests control_master / agent_forwarding ──────────────────────────────

#[test]
fn test_resolve_control_master_from_defaults() {
    let config = Config {
        defaults: Some(Defaults {
            control_master: Some(true),
            agent_forwarding: Some(true),
            control_path: Some("~/.ssh/ctl/%h_%p_%r".to_string()),
            control_persist: Some("30m".to_string()),
            ..Default::default()
        }),
        groups: vec![ConfigEntry::Group(make_group_with_env(
            "G",
            vec![make_env("E", vec![make_server("srv", "10.0.0.1", None)])],
        ))],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    let s = &resolved[0];
    assert!(s.control_master, "control_master should be true");
    assert!(s.agent_forwarding, "agent_forwarding should be true");
    assert!(
        !s.control_path.is_empty(),
        "control_path should be set when control_master=true"
    );
    assert_eq!(s.control_persist, "30m");
}

#[test]
fn test_resolve_control_master_false_by_default() {
    let config = Config {
        defaults: None,
        groups: vec![ConfigEntry::Group(make_group_with_env(
            "G",
            vec![make_env("E", vec![make_server("srv", "10.0.0.1", None)])],
        ))],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    assert!(!resolved[0].control_master);
    assert!(!resolved[0].agent_forwarding);
    assert!(resolved[0].control_path.is_empty());
}

// ─── Tests hooks ──────────────────────────────────────────────────────────

#[test]
fn test_resolve_hooks_from_defaults() {
    let config = Config {
        defaults: Some(Defaults {
            pre_connect_hook: Some("/hooks/pre.sh".to_string()),
            post_disconnect_hook: Some("/hooks/post.sh".to_string()),
            hook_timeout_secs: Some(15),
            ..Default::default()
        }),
        groups: vec![ConfigEntry::Group(make_group_with_env(
            "G",
            vec![make_env("E", vec![make_server("srv", "10.0.0.1", None)])],
        ))],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    let s = &resolved[0];
    assert_eq!(s.pre_connect_hook.as_deref(), Some("/hooks/pre.sh"));
    assert_eq!(s.post_disconnect_hook.as_deref(), Some("/hooks/post.sh"));
    assert_eq!(s.hook_timeout_secs, 15);
}

#[test]
fn test_resolve_server_hook_overrides_defaults() {
    let config = Config {
        defaults: Some(Defaults {
            pre_connect_hook: Some("/hooks/global.sh".to_string()),
            ..Default::default()
        }),
        groups: vec![ConfigEntry::Group(make_group_with_env(
            "G",
            vec![make_env(
                "E",
                vec![Server {
                    name: "srv".to_string(),
                    host: "10.0.0.1".to_string(),
                    pre_connect_hook: Some("/hooks/server.sh".to_string()),
                    ..Default::default()
                }],
            )],
        ))],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    assert_eq!(
        resolved[0].pre_connect_hook.as_deref(),
        Some("/hooks/server.sh"),
        "server-level hook should override defaults"
    );
}

#[test]
fn test_resolve_hooks_absent_by_default() {
    let config = Config {
        defaults: None,
        groups: vec![ConfigEntry::Group(make_group_with_env(
            "G",
            vec![make_env("E", vec![make_server("srv", "10.0.0.1", None)])],
        ))],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    assert!(resolved[0].pre_connect_hook.is_none());
    assert!(resolved[0].post_disconnect_hook.is_none());
    assert_eq!(resolved[0].hook_timeout_secs, 5);
}

// ─── Tests ssh_cert / ssh_agent_sock ─────────────────────────────────────

#[test]
fn test_resolve_ssh_cert_from_defaults() {
    let config = Config {
        defaults: Some(Defaults {
            ssh_cert: Some("/certs/id_ed25519-cert.pub".to_string()),
            ..Default::default()
        }),
        groups: vec![ConfigEntry::Group(make_group_with_env(
            "G",
            vec![make_env("E", vec![make_server("srv", "10.0.0.1", None)])],
        ))],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    assert_eq!(resolved[0].ssh_cert, "/certs/id_ed25519-cert.pub");
}

#[test]
fn test_resolve_ssh_agent_sock_from_defaults() {
    let config = Config {
        defaults: Some(Defaults {
            ssh_agent_sock: Some("/run/user/1000/gnupg/S.gpg-agent.ssh".to_string()),
            ..Default::default()
        }),
        groups: vec![ConfigEntry::Group(make_group_with_env(
            "G",
            vec![make_env("E", vec![make_server("srv", "10.0.0.1", None)])],
        ))],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    assert_eq!(
        resolved[0].ssh_agent_sock,
        "/run/user/1000/gnupg/S.gpg-agent.ssh"
    );
}

#[test]
fn test_resolve_ssh_agent_sock_server_overrides_defaults() {
    let config = Config {
        defaults: Some(Defaults {
            ssh_agent_sock: Some("/run/global.sock".to_string()),
            ..Default::default()
        }),
        groups: vec![ConfigEntry::Group(make_group_with_env(
            "G",
            vec![make_env(
                "E",
                vec![Server {
                    name: "srv".to_string(),
                    host: "10.0.0.1".to_string(),
                    ssh_agent_sock: Some("/run/per-server.sock".to_string()),
                    ..Default::default()
                }],
            )],
        ))],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    assert_eq!(resolved[0].ssh_agent_sock, "/run/per-server.sock");
}

// ─── Tests use_system_ssh_config ─────────────────────────────────────────

#[test]
fn test_resolve_use_system_ssh_config_false_by_default() {
    let config = Config {
        defaults: None,
        groups: vec![ConfigEntry::Group(make_group_with_env(
            "G",
            vec![make_env("E", vec![make_server("srv", "10.0.0.1", None)])],
        ))],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    assert!(!resolved[0].use_system_ssh_config);
}

#[test]
fn test_resolve_use_system_ssh_config_true_from_defaults() {
    let config = Config {
        defaults: Some(Defaults {
            use_system_ssh_config: Some(true),
            ..Default::default()
        }),
        groups: vec![ConfigEntry::Group(make_group_with_env(
            "G",
            vec![make_env("E", vec![make_server("srv", "10.0.0.1", None)])],
        ))],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    assert!(resolved[0].use_system_ssh_config);
}

// ─── Tests wallix field defaults ─────────────────────────────────────────

#[test]
fn test_resolve_wallix_field_defaults_applied() {
    let config = Config {
        defaults: Some(Defaults {
            mode: Some(ConnectionMode::Wallix),
            wallix: Some(BastionConfig {
                host: Some("bastion.example.com".to_string()),
                user: Some("buser".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        }),
        groups: vec![ConfigEntry::Group(make_group_with_env(
            "G",
            vec![make_env(
                "E",
                vec![make_server("srv", "target.example.com", None)],
            )],
        ))],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    let s = &resolved[0];
    assert!(s.wallix_auto_select, "auto_select should default to true");
    assert!(
        s.wallix_fail_if_menu_match_error,
        "fail_if_menu_match_error should default to true"
    );
    assert_eq!(s.wallix_selection_timeout_secs, 8);
    assert!(!s.wallix_direct, "direct should default to false");
    assert!(s.wallix_authorization.is_none());
    assert_eq!(s.wallix_account, "default");
    assert_eq!(s.wallix_protocol, "SSH");
}

#[test]
fn test_resolve_wallix_direct_and_authorization() {
    let config = Config {
        defaults: Some(Defaults {
            mode: Some(ConnectionMode::Wallix),
            wallix: Some(BastionConfig {
                host: Some("bastion.example.com".to_string()),
                user: Some("buser".to_string()),
                direct: Some(true),
                authorization: Some("STI-TEAM_prod-admins".to_string()),
                auto_select: Some(false),
                selection_timeout_secs: Some(3),
                ..Default::default()
            }),
            ..Default::default()
        }),
        groups: vec![ConfigEntry::Group(make_group_with_env(
            "G",
            vec![make_env(
                "E",
                vec![make_server("srv", "target.example.com", None)],
            )],
        ))],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    let s = &resolved[0];
    assert!(s.wallix_direct);
    assert_eq!(
        s.wallix_authorization.as_deref(),
        Some("STI-TEAM_prod-admins")
    );
    assert!(!s.wallix_auto_select);
    assert_eq!(s.wallix_selection_timeout_secs, 3);
}

#[test]
fn test_resolve_wallix_header_columns_custom() {
    let config = Config {
        defaults: Some(Defaults {
            mode: Some(ConnectionMode::Wallix),
            wallix: Some(BastionConfig {
                host: Some("bastion.example.com".to_string()),
                user: Some("buser".to_string()),
                header_columns: Some(vec![
                    "ID".to_string(),
                    "Target".to_string(),
                    "Auth".to_string(),
                ]),
                ..Default::default()
            }),
            ..Default::default()
        }),
        groups: vec![ConfigEntry::Group(make_group_with_env(
            "G",
            vec![make_env(
                "E",
                vec![make_server("srv", "target.example.com", None)],
            )],
        ))],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    assert_eq!(
        resolved[0].wallix_header_columns,
        vec!["ID", "Target", "Auth"]
    );
}

// ─── Tests jump multi-hop ─────────────────────────────────────────────────

#[test]
fn test_resolve_jump_multihop_string_format() {
    let config = Config {
        defaults: Some(Defaults {
            mode: Some(ConnectionMode::Jump),
            jump: Some(vec![
                JumpConfig {
                    host: Some("hop1.example.com".to_string()),
                    user: Some("jump1".to_string()),
                },
                JumpConfig {
                    host: Some("hop2.example.com".to_string()),
                    user: Some("jump2".to_string()),
                },
            ]),
            ..Default::default()
        }),
        groups: vec![ConfigEntry::Group(make_group_with_env(
            "G",
            vec![make_env("E", vec![make_server("srv", "10.0.0.1", None)])],
        ))],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    assert_eq!(
        resolved[0].jump_host.as_deref(),
        Some("jump1@hop1.example.com,jump2@hop2.example.com")
    );
}

#[test]
fn test_resolve_jump_server_overrides_defaults() {
    let config = Config {
        defaults: Some(Defaults {
            mode: Some(ConnectionMode::Jump),
            jump: Some(vec![JumpConfig {
                host: Some("default-jump.example.com".to_string()),
                user: Some("default-jump-user".to_string()),
            }]),
            ..Default::default()
        }),
        groups: vec![ConfigEntry::Group(make_group_with_env(
            "G",
            vec![make_env(
                "E",
                vec![Server {
                    name: "srv".to_string(),
                    host: "10.0.0.1".to_string(),
                    jump: Some(vec![JumpConfig {
                        host: Some("custom-jump.example.com".to_string()),
                        user: Some("custom-user".to_string()),
                    }]),
                    ..Default::default()
                }],
            )],
        ))],
        includes: vec![],
        vars: Default::default(),
    };

    let resolved = config.resolve().unwrap();
    assert_eq!(
        resolved[0].jump_host.as_deref(),
        Some("custom-user@custom-jump.example.com"),
        "server-level jump should replace defaults jump"
    );
}

// ─── Tests validate_yaml at group and environment level ───────────────────

#[test]
fn test_validation_unknown_group_field() {
    let yaml = r#"
groups:
  - name: G
    typo_group_field: true
    servers: []
"#;
    let warnings = validate_yaml(yaml, "test.yml");
    assert!(
        warnings.iter().any(|w| w.field == "typo_group_field"),
        "expected ValidationWarning for typo_group_field, got: {:?}",
        warnings
    );
}

#[test]
fn test_validation_unknown_env_field() {
    let yaml = r#"
groups:
  - name: G
    environments:
      - name: E
        bad_env_key: "oops"
        servers: []
"#;
    let warnings = validate_yaml(yaml, "test.yml");
    assert!(
        warnings.iter().any(|w| w.field == "bad_env_key"),
        "expected ValidationWarning for bad_env_key, got: {:?}",
        warnings
    );
}

// ── ConnectionMode tests ──────────────────────────────────────────────────

#[test]
fn test_connection_mode_next_cycles_all_three() {
    assert_eq!(ConnectionMode::Direct.next(), ConnectionMode::Jump);
    assert_eq!(ConnectionMode::Jump.next(), ConnectionMode::Wallix);
    assert_eq!(ConnectionMode::Wallix.next(), ConnectionMode::Direct);
}

#[test]
fn test_connection_mode_from_index_round_trip() {
    for i in 0..3usize {
        let mode = ConnectionMode::from_index(i);
        assert_eq!(mode.index(), i);
    }
}

#[test]
fn test_connection_mode_from_index_unknown_gives_direct() {
    assert_eq!(ConnectionMode::from_index(99), ConnectionMode::Direct);
}

#[test]
fn test_connection_mode_display() {
    assert_eq!(ConnectionMode::Direct.to_string(), "direct");
    assert_eq!(ConnectionMode::Jump.to_string(), "jump");
    assert_eq!(ConnectionMode::Wallix.to_string(), "wallix");
}

// ── extend_tags unit tests ────────────────────────────────────────────────

#[test]
fn test_extend_tags_deduplication() {
    let parent = vec!["prod".to_string(), "web".to_string()];
    let child = vec!["web".to_string(), "db".to_string()];
    let merged = extend_tags(Some(&parent), Some(&child));
    assert_eq!(merged, vec!["prod", "web", "db"]);
}

#[test]
fn test_extend_tags_parent_only() {
    let parent = vec!["a".to_string(), "b".to_string()];
    let merged = extend_tags(Some(&parent), None);
    assert_eq!(merged, vec!["a", "b"]);
}

#[test]
fn test_extend_tags_child_only() {
    let child = vec!["x".to_string()];
    let merged = extend_tags(None, Some(&child));
    assert_eq!(merged, vec!["x"]);
}

#[test]
fn test_extend_tags_both_none_empty() {
    assert!(extend_tags(None, None).is_empty());
}

// ── Phase A security tests ────────────────────────────────────────────────

#[test]
fn test_load_rejects_oversized_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big.yaml");
    // Write a file slightly over 5 MiB
    let content = "groups: []\n".repeat((MAX_FILE_SIZE_BYTES as usize / 10) + 1);
    std::fs::write(&path, content).unwrap();
    let err = Config::load_merged(&path, &mut HashSet::new(), 0).unwrap_err();
    assert!(
        matches!(err, ConfigError::FileTooLarge { .. }),
        "expected FileTooLarge, got: {err}"
    );
}

#[test]
fn test_load_accepts_file_under_limit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("small.yaml");
    std::fs::write(&path, "groups: []\n").unwrap();
    assert!(Config::load_merged(&path, &mut HashSet::new(), 0).is_ok());
}

#[test]
fn test_include_depth_exceeded() {
    let dir = tempfile::tempdir().unwrap();
    // Build a chain of MAX_INCLUDE_DEPTH + 2 files: A → B → … → Z
    let depth = (MAX_INCLUDE_DEPTH + 2) as usize;
    let paths: Vec<std::path::PathBuf> = (0..depth)
        .map(|i| dir.path().join(format!("depth_{i}.yaml")))
        .collect();
    // Last file: leaf with no includes
    std::fs::write(paths.last().unwrap(), "groups: []\n").unwrap();
    // Each file includes the next
    for i in (0..depth - 1).rev() {
        let next = paths[i + 1].file_name().unwrap().to_string_lossy();
        std::fs::write(
            &paths[i],
            format!("groups: []\nincludes:\n  - label: \"sub\"\n    path: \"{next}\"\n"),
        )
        .unwrap();
    }
    let err = Config::load_merged(&paths[0], &mut HashSet::new(), 0).unwrap_err();
    assert!(
        matches!(err, ConfigError::IncludeDepthExceeded { .. }),
        "expected IncludeDepthExceeded, got: {err}"
    );
}

#[test]
fn test_include_depth_at_limit() {
    let dir = tempfile::tempdir().unwrap();
    // Build a chain of exactly MAX_INCLUDE_DEPTH files: depth 0 calls load_merged
    // recursively; at depth MAX_INCLUDE_DEPTH the call still succeeds.
    let depth = MAX_INCLUDE_DEPTH as usize;
    let paths: Vec<std::path::PathBuf> = (0..=depth)
        .map(|i| dir.path().join(format!("ok_{i}.yaml")))
        .collect();
    std::fs::write(paths.last().unwrap(), "groups: []\n").unwrap();
    for i in (0..depth).rev() {
        let next = paths[i + 1].file_name().unwrap().to_string_lossy();
        std::fs::write(
            &paths[i],
            format!("groups: []\nincludes:\n  - label: \"sub\"\n    path: \"{next}\"\n"),
        )
        .unwrap();
    }
    assert!(
        Config::load_merged(&paths[0], &mut HashSet::new(), 0).is_ok(),
        "chain of {depth} levels should not exceed limit of {MAX_INCLUDE_DEPTH}"
    );
}

#[test]
fn test_include_two_levels_of_nesting_allowed() {
    // main → A → B (B a des serveurs, pas d'includes) : exactement 2 niveaux
    // d'includes imbriqués, doit réussir.
    let leaf_yaml = r#"
groups:
  - name: Leaf
    servers:
      - name: leaf_srv
        host: "203.0.113.9"
"#;
    let leaf_file = write_temp_yaml(leaf_yaml);

    let sub_yaml = format!(
        r#"
includes:
  - label: "B"
    path: "{}"
groups: []
"#,
        leaf_file.path().to_string_lossy()
    );
    let sub_file = write_temp_yaml(&sub_yaml);

    let main_yaml = format!(
        r#"
includes:
  - label: "A"
    path: "{}"
groups: []
"#,
        sub_file.path().to_string_lossy()
    );
    let main_file = write_temp_yaml(&main_yaml);

    let (config, warnings, _val) =
        Config::load_merged(main_file.path(), &mut std::collections::HashSet::new(), 0).unwrap();
    assert!(
        warnings.is_empty(),
        "Expected no warnings, got: {:?}",
        warnings
    );
    let resolved = config.resolve().unwrap();
    assert!(
        resolved
            .iter()
            .any(|s| s.name == "leaf_srv" && s.namespace == "A / B")
    );
}

#[test]
fn test_include_three_levels_of_nesting_rejected() {
    // main → A → B → C : un 3ᵉ niveau d'includes imbriqués dépasse la limite
    // (2) et doit être rejeté avec une erreur explicite, pas un avertissement.
    let leaf_yaml = "groups: []\n";
    let leaf_file = write_temp_yaml(leaf_yaml);

    let c_yaml = format!(
        r#"
includes:
  - label: "C"
    path: "{}"
groups: []
"#,
        leaf_file.path().to_string_lossy()
    );
    let c_file = write_temp_yaml(&c_yaml);

    let b_yaml = format!(
        r#"
includes:
  - label: "B"
    path: "{}"
groups: []
"#,
        c_file.path().to_string_lossy()
    );
    let b_file = write_temp_yaml(&b_yaml);

    let a_yaml = format!(
        r#"
includes:
  - label: "A"
    path: "{}"
groups: []
"#,
        b_file.path().to_string_lossy()
    );
    let a_file = write_temp_yaml(&a_yaml);

    let err =
        Config::load_merged(a_file.path(), &mut std::collections::HashSet::new(), 0).unwrap_err();
    assert!(
        matches!(err, ConfigError::IncludeDepthExceeded { .. }),
        "expected IncludeDepthExceeded, got: {err}"
    );
}

#[test]
fn test_fetch_url_rejects_http_scheme() {
    // https_only(true) rejects http:// before any network call — no mocking needed.
    let result = fetch_url("http://example.com/config.yaml");
    assert!(result.is_err(), "http:// URL should be rejected");
    let msg = result.unwrap_err();
    assert!(
        msg.to_lowercase().contains("http")
            || msg.to_lowercase().contains("https")
            || msg.to_lowercase().contains("plain"),
        "error message should mention the scheme issue, got: {msg}"
    );
}

// ─── Tests git_outdated_warnings (issue #196) ─────────────────────────────────

fn run_git(dir: &std::path::Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("failed to spawn git");
    assert!(
        output.status.success(),
        "git -C {} {:?} failed: {}",
        dir.display(),
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn init_repo_with_commit(dir: &std::path::Path) {
    std::fs::create_dir_all(dir).unwrap();
    run_git(dir, &["init", "-b", "main"]);
    run_git(dir, &["config", "user.email", "test@example.com"]);
    run_git(dir, &["config", "user.name", "Test"]);
    std::fs::write(dir.join("susshi.yml"), "groups: []\n").unwrap();
    run_git(dir, &["add", "susshi.yml"]);
    run_git(dir, &["commit", "-m", "initial"]);
}

fn namespace_for(label: &str, source_path: &std::path::Path) -> ConfigEntry {
    ConfigEntry::Namespace(NamespaceEntry {
        label: label.to_string(),
        source_path: source_path.display().to_string(),
        defaults: None,
        entries: vec![],
        vars: HashMap::new(),
    })
}

#[test]
fn test_git_outdated_warns_when_behind() {
    let dir = tempfile::tempdir().unwrap();
    let bare = dir.path().join("remote.git");
    let checkout = dir.path().join("checkout");
    let other = dir.path().join("other");

    run_git(dir.path(), &["init", "--bare", bare.to_str().unwrap()]);

    init_repo_with_commit(&checkout);
    run_git(
        &checkout,
        &["remote", "add", "origin", bare.to_str().unwrap()],
    );
    run_git(&checkout, &["push", "-u", "origin", "main"]);

    // Second clone pushes an extra commit that `checkout` doesn't have yet.
    run_git(
        dir.path(),
        &["clone", bare.to_str().unwrap(), other.to_str().unwrap()],
    );
    // The bare repo's default HEAD may not point at "main" (it depends on
    // git's default-branch config at `init --bare` time, not on what was
    // pushed) — check out the branch explicitly via its remote-tracking ref.
    run_git(&other, &["checkout", "main"]);
    run_git(&other, &["config", "user.email", "test@example.com"]);
    run_git(&other, &["config", "user.name", "Test"]);
    std::fs::write(other.join("extra.txt"), "x").unwrap();
    run_git(&other, &["add", "extra.txt"]);
    run_git(&other, &["commit", "-m", "extra"]);
    run_git(&other, &["push", "origin", "main"]);

    // Fetch (setup only — not part of the code under test) so `checkout`'s
    // remote-tracking ref knows about the new commit without pulling it in.
    run_git(&checkout, &["fetch", "origin"]);

    let config = Config {
        defaults: None,
        groups: vec![namespace_for("Sub", &checkout.join("susshi.yml"))],
        includes: vec![],
        vars: HashMap::new(),
    };

    let warnings = git_outdated_warnings(&config);
    assert_eq!(
        warnings.len(),
        1,
        "expected exactly one warning, got: {warnings:?}"
    );
    match &warnings[0] {
        IncludeWarning::GitOutdated { behind, .. } => assert_eq!(*behind, 1),
        other => panic!("expected GitOutdated, got: {other:?}"),
    }
}

#[test]
fn test_git_outdated_no_warning_when_up_to_date() {
    let dir = tempfile::tempdir().unwrap();
    let checkout = dir.path().join("checkout");
    init_repo_with_commit(&checkout);

    let config = Config {
        defaults: None,
        groups: vec![namespace_for("Sub", &checkout.join("susshi.yml"))],
        includes: vec![],
        vars: HashMap::new(),
    };

    assert!(git_outdated_warnings(&config).is_empty());
}

#[test]
fn test_git_outdated_no_warning_without_upstream() {
    let dir = tempfile::tempdir().unwrap();
    let checkout = dir.path().join("checkout");
    // Repo with a commit but no remote configured at all.
    init_repo_with_commit(&checkout);

    let config = Config {
        defaults: None,
        groups: vec![namespace_for("Sub", &checkout.join("susshi.yml"))],
        includes: vec![],
        vars: HashMap::new(),
    };

    assert!(git_outdated_warnings(&config).is_empty());
}

#[test]
fn test_git_outdated_no_warning_for_non_git_directory() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("susshi.yml"), "groups: []\n").unwrap();

    let config = Config {
        defaults: None,
        groups: vec![namespace_for("Sub", &dir.path().join("susshi.yml"))],
        includes: vec![],
        vars: HashMap::new(),
    };

    assert!(git_outdated_warnings(&config).is_empty());
}

#[test]
fn test_git_outdated_skips_http_includes() {
    let config = Config {
        defaults: None,
        groups: vec![namespace_for(
            "Remote",
            std::path::Path::new("https://example.com/susshi.yml"),
        )],
        includes: vec![],
        vars: HashMap::new(),
    };

    assert!(git_outdated_warnings(&config).is_empty());
}

#[test]
fn test_git_outdated_dedups_same_repo() {
    let dir = tempfile::tempdir().unwrap();
    let bare = dir.path().join("remote.git");
    let checkout = dir.path().join("checkout");
    let other = dir.path().join("other");

    run_git(dir.path(), &["init", "--bare", bare.to_str().unwrap()]);

    init_repo_with_commit(&checkout);
    std::fs::write(checkout.join("second.yml"), "groups: []\n").unwrap();
    run_git(&checkout, &["add", "second.yml"]);
    run_git(&checkout, &["commit", "-m", "second file"]);
    run_git(
        &checkout,
        &["remote", "add", "origin", bare.to_str().unwrap()],
    );
    run_git(&checkout, &["push", "-u", "origin", "main"]);

    run_git(
        dir.path(),
        &["clone", bare.to_str().unwrap(), other.to_str().unwrap()],
    );
    // The bare repo's default HEAD may not point at "main" (it depends on
    // git's default-branch config at `init --bare` time, not on what was
    // pushed) — check out the branch explicitly via its remote-tracking ref.
    run_git(&other, &["checkout", "main"]);
    run_git(&other, &["config", "user.email", "test@example.com"]);
    run_git(&other, &["config", "user.name", "Test"]);
    std::fs::write(other.join("extra.txt"), "x").unwrap();
    run_git(&other, &["add", "extra.txt"]);
    run_git(&other, &["commit", "-m", "extra"]);
    run_git(&other, &["push", "origin", "main"]);

    run_git(&checkout, &["fetch", "origin"]);

    // Two includes from the same checkout must yield a single warning.
    let config = Config {
        defaults: None,
        groups: vec![
            namespace_for("Sub A", &checkout.join("susshi.yml")),
            namespace_for("Sub B", &checkout.join("second.yml")),
        ],
        includes: vec![],
        vars: HashMap::new(),
    };

    let warnings = git_outdated_warnings(&config);
    assert_eq!(
        warnings.len(),
        1,
        "expected deduped warning, got: {warnings:?}"
    );
}
