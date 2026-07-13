use super::tests_helpers::make_namespace_config;
use super::*;
use crate::config::{ConfigEntry, Group, Server};

fn make_simple_config() -> Config {
    Config {
        defaults: None,
        includes: vec![],
        groups: vec![ConfigEntry::Group(Group {
            name: "Prod".to_string(),
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
                name: "web-01".to_string(),
                host: "203.0.113.10".to_string(),
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
        vars: Default::default(),
    }
}

// ── expansion_state ──────────────────────────────────────────────────────────

#[test]
fn collapse_all_clears_expanded_and_resets_selection() {
    let config = make_simple_config();
    let mut app = tests_helpers::make_test_app(config);

    // Expand "Prod" group
    app.expanded_items.insert("Group:Prod".to_string());
    app.selected_index = 2;

    app.collapse_all();

    assert!(app.expanded_items.is_empty());
    assert_eq!(app.selected_index, 0);
}

#[test]
fn collapse_all_marks_items_dirty() {
    let config = make_simple_config();
    let mut app = tests_helpers::make_test_app(config);

    app.items_dirty = false;
    app.collapse_all();

    assert!(app.items_dirty);
}

#[test]
fn invalidate_cache_sets_dirty() {
    let config = make_simple_config();
    let mut app = tests_helpers::make_test_app(config);

    app.items_dirty = false;
    app.invalidate_cache();

    assert!(app.items_dirty);
}

// ── favorites ────────────────────────────────────────────────────────────────

#[test]
fn toggle_favorites_view_flips_flag_and_resets_index() {
    let config = make_namespace_config();
    let mut app = tests_helpers::make_test_app(config);

    assert!(!app.favorites_only);
    app.toggle_favorites_view();
    assert!(app.favorites_only);

    app.selected_index = 3;
    app.toggle_favorites_view();
    assert!(!app.favorites_only);
    assert_eq!(app.selected_index, 0);
}

#[test]
fn toggle_favorites_view_marks_items_dirty() {
    let config = make_simple_config();
    let mut app = tests_helpers::make_test_app(config);

    app.items_dirty = false;
    app.toggle_favorites_view();
    assert!(app.items_dirty);
}

#[test]
fn record_connection_stores_timestamp() {
    let config = make_simple_config();
    let mut app = tests_helpers::make_test_app(config);

    let server = app.resolved_servers.first().cloned().unwrap();
    assert!(app.last_seen_for(&server).is_none());

    app.record_connection(&server);

    assert!(app.last_seen_for(&server).is_some());
}

#[test]
fn record_connection_timestamp_is_recent() {
    let config = make_simple_config();
    let mut app = tests_helpers::make_test_app(config);

    let server = app.resolved_servers.first().cloned().unwrap();
    app.record_connection(&server);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let ts = app.last_seen_for(&server).unwrap();

    assert!(ts <= now);
    assert!(now - ts < 5);
}

// ── expansion_state: expand_all ──────────────────────────────────────────────

#[test]
fn expand_all_inserts_group_keys() {
    let config = make_simple_config();
    let mut app = tests_helpers::make_test_app(config);
    app.expanded_items.clear();

    app.expand_all();

    // make_simple_config has Group "Prod" with no environments.
    assert!(app.expanded_items.contains("Group:Prod"));
    assert!(app.items_dirty);
}

#[test]
fn expand_all_inserts_namespace_and_env_keys() {
    let config = make_namespace_config();
    let mut app = tests_helpers::make_test_app(config);
    app.expanded_items.clear();

    app.expand_all();

    // make_namespace_config has a RootGroup and a "CES" namespace with CES_Group.
    assert!(app.expanded_items.contains("Group:RootGroup"));
    assert!(app.expanded_items.contains("NS:CES"));
    assert!(app.expanded_items.contains("NS:CES:Group:CES_Group"));
}

#[test]
fn expand_all_marks_dirty() {
    let config = make_simple_config();
    let mut app = tests_helpers::make_test_app(config);
    app.items_dirty = false;

    app.expand_all();

    assert!(app.items_dirty);
}

// ── favorites: toggle_favorite / is_selected_favorite ───────────────────────

fn make_app_with_selected_server() -> App {
    let config = make_simple_config();
    let mut app = tests_helpers::make_test_app(config);
    app.favorites.clear();
    app.expanded_items.insert("Group:Prod".to_string());
    app.items_dirty = true;
    app.get_visible_items();
    app.select(1); // select "web-01" (index 1 after group header)
    app
}

#[test]
fn toggle_favorite_adds_server() {
    let mut app = make_app_with_selected_server();
    assert!(!app.is_selected_favorite());
    app.toggle_favorite();
    assert!(app.is_selected_favorite());
}

#[test]
fn toggle_favorite_removes_server() {
    let mut app = make_app_with_selected_server();
    app.toggle_favorite(); // add
    assert!(app.is_selected_favorite());
    app.toggle_favorite(); // remove
    assert!(!app.is_selected_favorite());
}

#[test]
fn is_selected_favorite_returns_false_when_no_server() {
    let config = make_simple_config();
    let mut app = tests_helpers::make_test_app(config);
    app.favorites.clear();
    // No expansion → group header selected, not a server.
    assert!(!app.is_selected_favorite());
}
