use super::*;
use crate::config::{ConfigEntry, Group, Server};

fn make_simple_app() -> App {
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
            servers: Some(vec![Server {
                name: "s".to_string(),
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
            }]),
        })],
        vars: Default::default(),
    };
    tests_helpers::make_test_app(config)
}

// ── reset_cmd ────────────────────────────────────────────────────────────────

#[test]
fn reset_cmd_sets_idle() {
    let mut app = make_simple_app();
    app.cmd_state = CmdState::Running("ls".to_string());
    app.cmd_history_cursor = Some(2);

    app.reset_cmd();

    assert!(matches!(app.cmd_state, CmdState::Idle));
    assert!(app.cmd_rx.is_none());
    assert!(app.cmd_history_cursor.is_none());
}

#[test]
fn reset_cmd_clears_rx() {
    let mut app = make_simple_app();
    let (_tx, rx) = std::sync::mpsc::channel::<(String, bool)>();
    app.cmd_rx = Some(rx);
    app.cmd_state = CmdState::Done {
        cmd: "uptime".to_string(),
        output: "ok".to_string(),
        exit_ok: true,
    };

    app.reset_cmd();

    assert!(app.cmd_rx.is_none());
    assert!(matches!(app.cmd_state, CmdState::Idle));
}

// ── poll_cmd ─────────────────────────────────────────────────────────────────

#[test]
fn poll_cmd_no_rx_noop() {
    let mut app = make_simple_app();
    app.cmd_rx = None;
    // Should not panic and state stays Idle.
    app.poll_cmd();
    assert!(matches!(app.cmd_state, CmdState::Idle));
}

#[test]
fn poll_cmd_empty_channel_noop() {
    let mut app = make_simple_app();
    let (_tx, rx) = std::sync::mpsc::channel::<(String, bool)>();
    app.cmd_rx = Some(rx);
    app.cmd_state = CmdState::Running("ls".to_string());

    app.poll_cmd();

    // Nothing received yet → stays Running.
    assert!(matches!(app.cmd_state, CmdState::Running(_)));
}

#[test]
fn poll_cmd_receives_success_result() {
    let mut app = make_simple_app();
    let (tx, rx) = std::sync::mpsc::channel::<(String, bool)>();
    app.cmd_rx = Some(rx);
    app.cmd_state = CmdState::Running("uptime".to_string());

    tx.send(("load: 0.5".to_string(), true)).unwrap();
    drop(tx);

    app.poll_cmd();

    if let CmdState::Done {
        cmd,
        output,
        exit_ok,
    } = &app.cmd_state
    {
        assert_eq!(cmd, "uptime");
        assert_eq!(output, "load: 0.5");
        assert!(*exit_ok);
    } else {
        panic!("Expected Done state");
    }
    assert!(app.cmd_rx.is_none());
}

#[test]
fn poll_cmd_receives_failure_result() {
    let mut app = make_simple_app();
    let (tx, rx) = std::sync::mpsc::channel::<(String, bool)>();
    app.cmd_rx = Some(rx);
    app.cmd_state = CmdState::Running("badcmd".to_string());

    tx.send(("command not found".to_string(), false)).unwrap();

    app.poll_cmd();

    if let CmdState::Done { exit_ok, .. } = &app.cmd_state {
        assert!(!exit_ok);
    } else {
        panic!("Expected Done state");
    }
}
