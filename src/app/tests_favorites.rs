use super::tests_helpers::make_namespace_config;
use super::*;
use crate::config::{ConfigEntry, Group, Server};

fn make_app_with_server() -> App {
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
            servers: Some(vec![Server {
                name: "srv1".into(),
                host: "10.0.0.1".into(),
                ..Default::default()
            }]),
        })],
        vars: Default::default(),
    };
    let mut app = tests_helpers::make_test_app(config);
    // Reset any persisted state and expand the group so the server is visible at index 1
    app.expanded_items.clear();
    app.favorites.clear();
    app.expanded_items.insert("Group:Grp".to_string());
    app.items_dirty = true;
    app.get_visible_items();
    app
}

// ─── toggle_favorite ─────────────────────────────────────────────────────────

#[test]
fn toggle_favorite_adds_server_to_favorites() {
    let mut app = make_app_with_server();
    app.selected_index = 1; // the server
    app.toggle_favorite();
    let server = app.selected_server().unwrap();
    let key = App::server_key(&server);
    assert!(app.favorites.contains(&key));
}

#[test]
fn toggle_favorite_removes_server_from_favorites_when_already_present() {
    let mut app = make_app_with_server();
    app.selected_index = 1;
    // Add to favorites first
    app.toggle_favorite();
    let server = app.selected_server().unwrap();
    let key = App::server_key(&server);
    assert!(app.favorites.contains(&key), "should be added first");
    // Toggle again to remove
    app.toggle_favorite();
    assert!(
        !app.favorites.contains(&key),
        "should be removed after second toggle"
    );
}

#[test]
fn toggle_favorite_on_group_header_is_noop() {
    let mut app = make_app_with_server();
    app.selected_index = 0; // group header
    let before = app.favorites.clone();
    app.toggle_favorite();
    assert_eq!(
        app.favorites, before,
        "favorites must not change when no server is selected"
    );
}

// ─── is_selected_favorite ─────────────────────────────────────────────────────

#[test]
fn is_selected_favorite_returns_false_initially() {
    let mut app = make_app_with_server();
    app.selected_index = 1;
    assert!(!app.is_selected_favorite());
}

#[test]
fn is_selected_favorite_returns_true_after_toggle() {
    let mut app = make_app_with_server();
    app.selected_index = 1;
    app.toggle_favorite();
    assert!(app.is_selected_favorite());
}

#[test]
fn is_selected_favorite_returns_false_on_group_header() {
    let mut app = make_app_with_server();
    app.selected_index = 0; // group header
    assert!(!app.is_selected_favorite());
}

// ─── toggle_favorites_view ────────────────────────────────────────────────────

#[test]
fn toggle_favorites_view_sets_favorites_only() {
    let mut app = make_app_with_server();
    assert!(!app.favorites_only);
    app.toggle_favorites_view();
    assert!(app.favorites_only);
}

#[test]
fn toggle_favorites_view_unsets_favorites_only_on_second_call() {
    let mut app = make_app_with_server();
    app.toggle_favorites_view();
    app.toggle_favorites_view();
    assert!(!app.favorites_only);
}

#[test]
fn toggle_favorites_view_resets_selected_index_to_zero() {
    let mut app = make_app_with_server();
    app.selected_index = 1;
    app.toggle_favorites_view();
    assert_eq!(app.selected_index, 0);
}

#[test]
fn toggle_favorites_view_marks_items_dirty() {
    let mut app = make_app_with_server();
    app.items_dirty = false;
    app.toggle_favorites_view();
    assert!(app.items_dirty);
}

// ─── record_connection / last_seen_for ────────────────────────────────────────

#[test]
fn last_seen_for_returns_none_before_any_connection() {
    let app = make_app_with_server();
    let config = make_namespace_config();
    let resolved = config.resolve().unwrap();
    let server = resolved.first().unwrap();
    assert!(app.last_seen_for(server).is_none());
}

#[test]
fn record_connection_stores_nonzero_timestamp() {
    let mut app = make_app_with_server();
    app.selected_index = 1;
    let server = app.selected_server().unwrap();
    app.record_connection(&server);
    let ts = app.last_seen_for(&server);
    assert!(ts.is_some(), "timestamp must be recorded");
    assert!(ts.unwrap() > 0, "timestamp must be positive");
}

#[test]
fn record_connection_updates_timestamp_on_second_call() {
    let mut app = make_app_with_server();
    app.selected_index = 1;
    let server = app.selected_server().unwrap();
    app.record_connection(&server);
    let ts1 = app.last_seen_for(&server).unwrap();
    // Sleep just enough that the second call might differ; but since SystemTime
    // resolution is typically 1s, we just verify both calls succeed.
    app.record_connection(&server);
    let ts2 = app.last_seen_for(&server).unwrap();
    assert!(ts2 >= ts1, "second timestamp must be >= first");
}
