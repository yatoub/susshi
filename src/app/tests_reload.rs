use super::*;
use crate::config::{ConfigEntry, Group, IncludeWarning, Server};

fn make_simple_config_for_reload() -> Config {
    Config {
        defaults: None,
        includes: vec![],
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
            production: None,
            servers: Some(vec![Server {
                name: "s1".to_string(),
                host: "10.0.0.1".to_string(),
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
                production: None,
                ..Default::default()
            }]),
        })],
        vars: Default::default(),
    }
}

// ── lifecycle: warnings set error mode ───────────────────────────────────────

#[test]
fn app_new_with_load_error_warning_sets_error_mode() {
    let config = make_simple_config_for_reload();
    let warning = IncludeWarning::LoadError {
        label: "prod".to_string(),
        path: "/fake/prod.yaml".to_string(),
        error: "file not found".to_string(),
    };
    let app = App::new(
        config,
        vec![warning],
        std::path::PathBuf::from("/fake"),
        vec![],
    )
    .unwrap();
    assert!(matches!(app.app_mode, AppMode::Error(_)));
}

#[test]
fn app_new_with_circular_warning_sets_error_mode() {
    let config = make_simple_config_for_reload();
    let warning = IncludeWarning::Circular {
        label: "loop".to_string(),
        path: "/fake/loop.yaml".to_string(),
    };
    let app = App::new(
        config,
        vec![warning],
        std::path::PathBuf::from("/fake"),
        vec![],
    )
    .unwrap();
    assert!(matches!(app.app_mode, AppMode::Error(_)));
}

#[test]
fn test_reload_detects_new_host_in_included_file() {
    use std::fs;

    let temp = tempfile::tempdir().unwrap();
    let main_path = temp.path().join("main.yaml");
    let include_path = temp.path().join("included.yaml");

    fs::write(
        &include_path,
        "groups:\n  - name: \"IncGroup\"\n    servers:\n      - name: \"inc-1\"\n        host: \"198.51.100.101\"\n",
    )
    .unwrap();

    fs::write(
        &main_path,
        "groups: []\nincludes:\n  - label: \"Included\"\n    path: \"included.yaml\"\n",
    )
    .unwrap();

    let (config, warnings, validation_warnings) =
        Config::load_merged(&main_path, &mut std::collections::HashSet::new(), 0).unwrap();
    assert!(warnings.is_empty());
    assert!(validation_warnings.is_empty());

    let mut app =
        App::new(config, vec![], main_path.clone(), vec![]).expect("app init should work");
    assert_eq!(app.resolved_servers.len(), 1);

    fs::write(
        &include_path,
        "groups:\n  - name: \"IncGroup\"\n    servers:\n      - name: \"inc-1\"\n        host: \"198.51.100.101\"\n      - name: \"inc-2\"\n        host: \"198.51.100.102\"\n",
    )
    .unwrap();

    app.reload().expect("reload should succeed");

    assert_eq!(app.resolved_servers.len(), 2);
    assert!(app.resolved_servers.iter().any(|s| s.name == "inc-2"));
}
