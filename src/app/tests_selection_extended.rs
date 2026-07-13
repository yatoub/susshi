use super::*;
use crate::config::{ConfigEntry, Group, Server};

fn make_app_two_servers() -> App {
    let config = Config {
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
            servers: Some(vec![
                Server {
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
                    ..Default::default()
                },
                Server {
                    name: "s2".to_string(),
                    host: "10.0.0.2".to_string(),
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
                },
            ]),
        })],
        vars: Default::default(),
    };
    let mut app = tests_helpers::make_test_app(config);
    // Expand so servers are visible: [Group:G, Server:s1, Server:s2]
    app.expanded_items.insert("Group:G".to_string());
    app.items_dirty = true;
    app.get_visible_items();
    app
}

// ── next / previous ──────────────────────────────────────────────────────────

#[test]
fn next_advances_selection() {
    let mut app = make_app_two_servers();
    app.select(0);
    app.next();
    assert_eq!(app.selected_index, 1);
}

#[test]
fn next_wraps_around_to_zero() {
    let mut app = make_app_two_servers();
    let count = app.get_visible_items().len(); // 3: group + 2 servers
    app.select(count - 1);
    app.next();
    assert_eq!(app.selected_index, 0);
}

#[test]
fn previous_decrements_selection() {
    let mut app = make_app_two_servers();
    app.select(2);
    app.previous();
    assert_eq!(app.selected_index, 1);
}

#[test]
fn previous_from_zero_wraps_to_last() {
    let mut app = make_app_two_servers();
    let count = app.get_visible_items().len();
    app.select(0);
    app.previous();
    assert_eq!(app.selected_index, count - 1);
}

// ── select ───────────────────────────────────────────────────────────────────

#[test]
fn select_out_of_bounds_noop() {
    let mut app = make_app_two_servers();
    app.select(0);
    app.select(999); // out of bounds
    assert_eq!(app.selected_index, 0);
}

#[test]
fn select_same_index_no_mode_update() {
    let mut app = make_app_two_servers();
    app.select(1); // first server
    let mode_before = app.connection_mode;
    app.select(1); // same index → should not change mode
    assert_eq!(app.connection_mode, mode_before);
}

// ── selected_server ──────────────────────────────────────────────────────────

#[test]
fn selected_server_returns_none_for_group_header() {
    let mut app = make_app_two_servers();
    app.select(0); // group header
    assert!(app.selected_server().is_none());
}

#[test]
fn selected_server_returns_server() {
    let mut app = make_app_two_servers();
    app.select(1); // first server
    let server = app.selected_server();
    assert!(server.is_some());
    assert_eq!(server.unwrap().name, "s1");
}

#[test]
fn selected_server_second_server() {
    let mut app = make_app_two_servers();
    app.select(2); // second server
    let server = app.selected_server().unwrap();
    assert_eq!(server.name, "s2");
}

// ── empty list edge cases ────────────────────────────────────────────────────

#[test]
fn next_empty_list_noop() {
    let config = Config {
        defaults: None,
        includes: vec![],
        groups: vec![],
        vars: Default::default(),
    };
    let mut app = tests_helpers::make_test_app(config);
    // Should not panic.
    app.next();
    app.previous();
}
