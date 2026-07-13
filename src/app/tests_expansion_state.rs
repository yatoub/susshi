use super::tests_helpers::make_namespace_env_config;
use super::*;
use crate::config::{ConfigEntry, Group, Server};

/// Reset the state loaded from disk so tests start from a known blank state.
fn clean(mut app: App) -> App {
    app.expanded_items.clear();
    app.items_dirty = true;
    app
}

fn make_app_with_group_and_env() -> App {
    clean(
        App::new(
            make_namespace_env_config(),
            vec![],
            std::path::PathBuf::from("/fake"),
            vec![],
        )
        .unwrap(),
    )
}

fn make_app_root_group_only() -> App {
    let config = Config {
        defaults: None,
        includes: vec![],
        groups: vec![ConfigEntry::Group(Group {
            name: "RootOnly".to_string(),
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
                name: "rs1".into(),
                host: "10.1.0.1".into(),
                ..Default::default()
            }]),
        })],
        vars: Default::default(),
    };
    clean(tests_helpers::make_test_app(config))
}

fn make_app_two_servers() -> App {
    let config = Config {
        defaults: None,
        includes: vec![],
        groups: vec![ConfigEntry::Group(Group {
            name: "Grp".to_string(),
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
            servers: Some(vec![
                Server {
                    name: "s1".into(),
                    host: "10.0.0.1".into(),
                    ..Default::default()
                },
                Server {
                    name: "s2".into(),
                    host: "10.0.0.2".into(),
                    ..Default::default()
                },
            ]),
        })],
        vars: Default::default(),
    };
    clean(tests_helpers::make_test_app(config))
}

// ─── expand_all ───────────────────────────────────────────────────────────────

#[test]
fn expand_all_inserts_namespace_and_group_and_env_ids() {
    let mut app = make_app_with_group_and_env();
    app.expand_all();
    assert!(
        app.expanded_items.contains("NS:NS1"),
        "namespace key expected"
    );
    assert!(
        app.expanded_items.contains("NS:NS1:Group:GrpA"),
        "namespaced group key expected"
    );
    assert!(
        app.expanded_items.contains("NS:NS1:Env:GrpA:EnvA"),
        "namespaced env key expected"
    );
}

#[test]
fn expand_all_root_group_no_ns_prefix() {
    let mut app = make_app_root_group_only();
    app.expand_all();
    assert!(
        app.expanded_items.contains("Group:RootOnly"),
        "root group key expected, got: {:?}",
        app.expanded_items
    );
    assert!(
        !app.expanded_items.iter().any(|k| k.starts_with("NS:")),
        "no NS: prefix expected for root-only group, got: {:?}",
        app.expanded_items
    );
}

#[test]
fn expand_all_marks_items_dirty() {
    let mut app = make_app_root_group_only();
    app.items_dirty = false;
    app.expand_all();
    assert!(app.items_dirty);
}

// ─── collapse_all ─────────────────────────────────────────────────────────────

#[test]
fn collapse_all_clears_expanded_items() {
    let mut app = make_app_with_group_and_env();
    app.expand_all();
    assert!(!app.expanded_items.is_empty());
    app.collapse_all();
    assert!(app.expanded_items.is_empty());
}

#[test]
fn collapse_all_resets_selected_index_to_zero() {
    let mut app = make_app_with_group_and_env();
    app.expand_all();
    app.selected_index = 2;
    app.collapse_all();
    assert_eq!(app.selected_index, 0);
}

#[test]
fn collapse_all_marks_items_dirty() {
    let mut app = make_app_root_group_only();
    app.items_dirty = false;
    app.collapse_all();
    assert!(app.items_dirty);
}

// ─── toggle_expansion ─────────────────────────────────────────────────────────

#[test]
fn toggle_expansion_on_group_expands_it() {
    let mut app = make_app_two_servers();
    // Index 0 is the Group header
    app.selected_index = 0;
    app.toggle_expansion();
    assert!(
        app.expanded_items.contains("Group:Grp"),
        "group should be expanded after toggle"
    );
}

#[test]
fn toggle_expansion_on_already_expanded_group_collapses_it() {
    let mut app = make_app_two_servers();
    app.expanded_items.insert("Group:Grp".to_string());
    app.items_dirty = true;
    app.selected_index = 0;
    app.toggle_expansion();
    assert!(
        !app.expanded_items.contains("Group:Grp"),
        "group should be collapsed after second toggle"
    );
}

#[test]
fn toggle_expansion_on_server_is_noop() {
    let mut app = make_app_two_servers();
    // Expand first so servers are visible
    app.expanded_items.insert("Group:Grp".to_string());
    app.items_dirty = true;
    app.get_visible_items(); // force rebuild

    let before = app.expanded_items.clone();
    app.selected_index = 1; // first server
    app.toggle_expansion();
    // expanded_items must not change (items_dirty will be set but that's fine)
    assert_eq!(
        app.expanded_items, before,
        "expanded_items must not change when toggling a server"
    );
}

#[test]
fn toggle_expansion_marks_items_dirty() {
    let mut app = make_app_two_servers();
    app.items_dirty = false;
    app.toggle_expansion();
    assert!(app.items_dirty);
}

// ─── invalidate_cache ─────────────────────────────────────────────────────────

#[test]
fn invalidate_cache_sets_items_dirty() {
    let mut app = make_app_root_group_only();
    app.items_dirty = false;
    app.invalidate_cache();
    assert!(app.items_dirty);
}
