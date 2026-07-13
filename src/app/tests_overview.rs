use super::*;
use crate::config::{ConfigEntry, Environment, Group, Server};

fn make_app_with_group() -> App {
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
            environments: Some(vec![Environment {
                name: "Env1".to_string(),
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
                    name: "srv1".to_string(),
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
                }],
            }]),
            servers: None,
            tunnels: None,
            tags: None,
        })],
        vars: Default::default(),
    };
    let mut app = tests_helpers::make_test_app(config);
    // Expand everything so all items are visible.
    app.expanded_items.insert("Group:Grp".to_string());
    app.expanded_items.insert("Env:Grp:Env1".to_string());
    app.items_dirty = true;
    app.get_visible_items();
    app
}

// ── close_overview ───────────────────────────────────────────────────────────

#[test]
fn close_overview_clears_state() {
    let mut app = make_app_with_group();
    app.overview = Some(OverviewState {
        group_name: "Grp".to_string(),
        entries: vec![],
        scroll: 0,
    });
    let (_, rx) = std::sync::mpsc::channel::<(usize, Result<crate::probe::ProbeResult, String>)>();
    app.overview_rx = Some(rx);

    app.close_overview();
    assert!(app.overview.is_none());
    assert!(app.overview_rx.is_none());
}

// ── open_overview ────────────────────────────────────────────────────────────

#[test]
fn open_overview_server_selected_noop() {
    let mut app = make_app_with_group();
    // Select the server (index 2: Group, Env, Server).
    app.select(2);
    let items = app.get_visible_items();
    assert!(matches!(items.get(2), Some(ConfigItem::Server(_))));

    app.open_overview();
    // Server selected → no overview opened.
    assert!(app.overview.is_none());
}

#[test]
fn open_overview_no_items_noop() {
    let config = Config {
        defaults: None,
        includes: vec![],
        groups: vec![],
        vars: Default::default(),
    };
    let mut app = tests_helpers::make_test_app(config);
    app.open_overview();
    assert!(app.overview.is_none());
}

#[test]
fn open_overview_group_opens_with_entries() {
    let mut app = make_app_with_group();
    // Select the group header (index 0).
    app.select(0);
    let items = app.get_visible_items();
    assert!(matches!(items.first(), Some(ConfigItem::Group(..))));

    app.open_overview();
    assert!(app.overview.is_some());
    let ov = app.overview.as_ref().unwrap();
    assert!(!ov.entries.is_empty());
    assert_eq!(ov.entries[0].server_name, "srv1");
}

#[test]
fn open_overview_env_opens_with_entries() {
    let mut app = make_app_with_group();
    // Select the environment header (index 1).
    app.select(1);
    let items = app.get_visible_items();
    assert!(matches!(items.get(1), Some(ConfigItem::Environment(..))));

    app.open_overview();
    assert!(app.overview.is_some());
    let ov = app.overview.as_ref().unwrap();
    assert!(ov.group_name.contains("Env1"));
}

// ── overview scroll ──────────────────────────────────────────────────────────

#[test]
fn overview_scroll_down_increments() {
    let mut app = make_app_with_group();
    app.overview = Some(OverviewState {
        group_name: "G".to_string(),
        entries: vec![],
        scroll: 0,
    });
    app.overview_scroll_down();
    assert_eq!(app.overview.as_ref().unwrap().scroll, 1);
    app.overview_scroll_down();
    assert_eq!(app.overview.as_ref().unwrap().scroll, 2);
}

#[test]
fn overview_scroll_up_decrements() {
    let mut app = make_app_with_group();
    app.overview = Some(OverviewState {
        group_name: "G".to_string(),
        entries: vec![],
        scroll: 3,
    });
    app.overview_scroll_up();
    assert_eq!(app.overview.as_ref().unwrap().scroll, 2);
}

#[test]
fn overview_scroll_up_saturates_at_zero() {
    let mut app = make_app_with_group();
    app.overview = Some(OverviewState {
        group_name: "G".to_string(),
        entries: vec![],
        scroll: 0,
    });
    app.overview_scroll_up();
    assert_eq!(app.overview.as_ref().unwrap().scroll, 0);
}

#[test]
fn overview_scroll_noop_when_closed() {
    let mut app = make_app_with_group();
    assert!(app.overview.is_none());
    // Should not panic.
    app.overview_scroll_down();
    app.overview_scroll_up();
}

// ── poll_overview ─────────────────────────────────────────────────────────────

#[test]
fn poll_overview_no_rx_noop() {
    let mut app = make_app_with_group();
    app.overview_rx = None;
    // Should not panic.
    app.poll_overview();
}

#[test]
fn poll_overview_delivers_ok_result() {
    let mut app = make_app_with_group();
    app.overview = Some(OverviewState {
        group_name: "G".to_string(),
        entries: vec![OverviewEntry {
            server_name: "srv1".to_string(),
            host: "10.0.0.1".to_string(),
            status: OverviewStatus::Pending,
        }],
        scroll: 0,
    });
    let (tx, rx) = std::sync::mpsc::channel::<(usize, Result<crate::probe::ProbeResult, String>)>();
    app.overview_rx = Some(rx);

    let probe_result = crate::probe::ProbeResult {
        load: "0.5".to_string(),
        ram_pct: 60,
        disk_pct: 40,
        profile: crate::probe::ProbeProfile::Standard,
        kernel: String::new(),
        cpu_model: String::new(),
        cpu_cores: 1,
        os_name: String::new(),
        ram_total_gb: 0.0,
        disk_total_gb: 0.0,
        extra_fs: vec![],
        notes: vec![],
        control_master_active: false,
    };
    tx.send((0, Ok(probe_result))).unwrap();
    drop(tx);

    app.poll_overview();

    let ov = app.overview.as_ref().unwrap();
    assert!(matches!(
        ov.entries[0].status,
        OverviewStatus::Ok {
            ram_pct: 60,
            disk_pct: 40,
            ..
        }
    ));
}

#[test]
fn poll_overview_delivers_error_result() {
    let mut app = make_app_with_group();
    app.overview = Some(OverviewState {
        group_name: "G".to_string(),
        entries: vec![OverviewEntry {
            server_name: "srv1".to_string(),
            host: "10.0.0.1".to_string(),
            status: OverviewStatus::Pending,
        }],
        scroll: 0,
    });
    let (tx, rx) = std::sync::mpsc::channel::<(usize, Result<crate::probe::ProbeResult, String>)>();
    app.overview_rx = Some(rx);

    tx.send((0, Err("timeout".to_string()))).unwrap();
    drop(tx);

    app.poll_overview();

    let ov = app.overview.as_ref().unwrap();
    assert!(matches!(&ov.entries[0].status, OverviewStatus::Error(e) if e == "timeout"));
}
