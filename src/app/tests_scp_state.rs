use super::*;
use crate::config::{ConfigEntry, Group, Server};
use crate::ssh::sftp::ScpDirection;

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
                name: "srv".to_string(),
                host: "10.0.0.1".to_string(),
                user: Some("admin".to_string()),
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
    let mut app = tests_helpers::make_test_app(config);
    app.expanded_items.insert("Group:Grp".to_string());
    app.items_dirty = true;
    app.get_visible_items();
    app.select(1);
    app
}

// ── open_scp_select_direction ────────────────────────────────────────────────

#[test]
fn open_scp_wallix_mode_noop() {
    let mut app = make_app_with_server();
    app.connection_mode = ConnectionMode::Wallix;
    app.open_scp_select_direction();
    assert!(matches!(app.scp_state, ScpState::Idle));
}

#[test]
fn open_scp_with_server_sets_selecting_direction() {
    let mut app = make_app_with_server();
    app.open_scp_select_direction();
    assert!(matches!(app.scp_state, ScpState::SelectingDirection));
}

// ── scp_select_direction ─────────────────────────────────────────────────────

#[test]
fn scp_select_direction_upload_prefills_form() {
    let mut app = make_app_with_server();
    app.scp_state = ScpState::SelectingDirection;
    app.scp_select_direction(ScpDirection::Upload);
    if let ScpState::FillingForm {
        direction,
        local,
        remote,
        focus,
        ..
    } = &app.scp_state
    {
        assert_eq!(*direction, ScpDirection::Upload);
        assert!(local.is_empty());
        assert!(remote.contains("10.0.0.1"));
        assert_eq!(*focus, ScpFormField::Local);
    } else {
        panic!("Expected FillingForm state");
    }
}

#[test]
fn scp_select_direction_download() {
    let mut app = make_app_with_server();
    app.scp_state = ScpState::SelectingDirection;
    app.scp_select_direction(ScpDirection::Download);
    assert!(matches!(
        app.scp_state,
        ScpState::FillingForm {
            direction: ScpDirection::Download,
            ..
        }
    ));
}

// ── close_scp_overlay ────────────────────────────────────────────────────────

#[test]
fn close_scp_overlay_idle() {
    let mut app = make_app_with_server();
    app.close_scp_overlay();
    assert!(matches!(app.scp_state, ScpState::Idle));
}

#[test]
fn close_scp_overlay_selecting_direction_goes_idle() {
    let mut app = make_app_with_server();
    app.scp_state = ScpState::SelectingDirection;
    app.close_scp_overlay();
    assert!(matches!(app.scp_state, ScpState::Idle));
}

#[test]
fn close_scp_overlay_running_is_noop() {
    let mut app = make_app_with_server();
    app.scp_state = ScpState::Running {
        direction: ScpDirection::Upload,
        label: "test.txt".to_string(),
        progress: 50,
        started_at: std::time::Instant::now(),
        file_size: 1024,
    };
    app.close_scp_overlay();
    // Running state must not be interrupted.
    assert!(matches!(app.scp_state, ScpState::Running { .. }));
}

// ── dismiss_scp_result ───────────────────────────────────────────────────────

#[test]
fn dismiss_scp_done_goes_idle() {
    let mut app = make_app_with_server();
    app.scp_state = ScpState::Done {
        direction: ScpDirection::Upload,
        exit_ok: true,
    };
    app.dismiss_scp_result();
    assert!(matches!(app.scp_state, ScpState::Idle));
}

#[test]
fn dismiss_scp_error_goes_idle() {
    let mut app = make_app_with_server();
    app.scp_state = ScpState::Error("oops".to_string());
    app.dismiss_scp_result();
    assert!(matches!(app.scp_state, ScpState::Idle));
}

#[test]
fn dismiss_scp_running_is_noop() {
    let mut app = make_app_with_server();
    app.scp_state = ScpState::Running {
        direction: ScpDirection::Upload,
        label: "f".to_string(),
        progress: 10,
        started_at: std::time::Instant::now(),
        file_size: 0,
    };
    app.dismiss_scp_result();
    assert!(matches!(app.scp_state, ScpState::Running { .. }));
}

// ── scp_form_char / backspace ────────────────────────────────────────────────

fn app_in_form(direction: ScpDirection, focus: ScpFormField) -> App {
    let mut app = make_app_with_server();
    app.scp_state = ScpState::FillingForm {
        direction,
        local: String::new(),
        remote: String::new(),
        focus,
        error: String::new(),
    };
    app
}

#[test]
fn scp_form_char_local_field() {
    let mut app = app_in_form(ScpDirection::Upload, ScpFormField::Local);
    app.scp_form_char('/');
    app.scp_form_char('t');
    app.scp_form_char('m');
    app.scp_form_char('p');
    if let ScpState::FillingForm { local, .. } = &app.scp_state {
        assert_eq!(local, "/tmp");
    } else {
        panic!("Expected FillingForm");
    }
}

#[test]
fn scp_form_char_remote_field() {
    let mut app = app_in_form(ScpDirection::Upload, ScpFormField::Remote);
    app.scp_form_char('~');
    app.scp_form_char('/');
    if let ScpState::FillingForm { remote, .. } = &app.scp_state {
        assert_eq!(remote, "~/");
    } else {
        panic!("Expected FillingForm");
    }
}

#[test]
fn scp_form_backspace_local() {
    let mut app = app_in_form(ScpDirection::Upload, ScpFormField::Local);
    app.scp_form_char('a');
    app.scp_form_char('b');
    app.scp_form_backspace();
    if let ScpState::FillingForm { local, .. } = &app.scp_state {
        assert_eq!(local, "a");
    } else {
        panic!("Expected FillingForm");
    }
}

#[test]
fn scp_form_backspace_remote() {
    let mut app = app_in_form(ScpDirection::Upload, ScpFormField::Remote);
    app.scp_form_char('x');
    app.scp_form_char('y');
    app.scp_form_backspace();
    if let ScpState::FillingForm { remote, .. } = &app.scp_state {
        assert_eq!(remote, "x");
    } else {
        panic!("Expected FillingForm");
    }
}

// ── scp_form_next_field ──────────────────────────────────────────────────────

#[test]
fn scp_form_next_field_toggles_focus() {
    let mut app = app_in_form(ScpDirection::Upload, ScpFormField::Local);
    app.scp_form_next_field();
    if let ScpState::FillingForm { focus, .. } = &app.scp_state {
        assert_eq!(*focus, ScpFormField::Remote);
    }
    app.scp_form_next_field();
    if let ScpState::FillingForm { focus, .. } = &app.scp_state {
        assert_eq!(*focus, ScpFormField::Local);
    }
}

// ── scp_form_submit validation ────────────────────────────────────────────────

#[test]
fn scp_form_submit_empty_local_sets_error() {
    let mut app = app_in_form(ScpDirection::Upload, ScpFormField::Local);
    // remote is non-empty, local is empty.
    if let ScpState::FillingForm { remote, .. } = &mut app.scp_state {
        *remote = "user@host:~/".to_string();
    }
    app.scp_form_submit();
    if let ScpState::FillingForm { error, .. } = &app.scp_state {
        assert!(!error.is_empty());
    } else {
        panic!("Expected FillingForm to stay open");
    }
}

#[test]
fn scp_form_submit_empty_remote_sets_error() {
    let mut app = app_in_form(ScpDirection::Upload, ScpFormField::Local);
    // local non-empty, remote empty.
    if let ScpState::FillingForm { local, .. } = &mut app.scp_state {
        *local = "/tmp/file.txt".to_string();
    }
    app.scp_form_submit();
    if let ScpState::FillingForm { error, .. } = &app.scp_state {
        assert!(!error.is_empty());
    } else {
        panic!("Expected FillingForm to stay open");
    }
}

// ── poll_scp_events ───────────────────────────────────────────────────────────

fn app_in_running(direction: ScpDirection) -> App {
    let mut app = make_app_with_server();
    app.scp_state = ScpState::Running {
        direction,
        label: "file.tar.gz".to_string(),
        progress: 0,
        started_at: std::time::Instant::now(),
        file_size: 0,
    };
    app
}

#[test]
fn poll_scp_events_no_rx_noop() {
    let mut app = make_app_with_server();
    // scp_rx is None → should not panic.
    app.poll_scp_events();
    assert!(matches!(app.scp_state, ScpState::Idle));
}

#[test]
fn poll_scp_events_with_disconnected_rx() {
    let mut app = make_app_with_server();
    let (tx, rx) = std::sync::mpsc::channel::<crate::ssh::sftp::ScpEvent>();
    drop(tx); // disconnect immediately
    app.scp_rx = Some(rx);
    app.scp_state = ScpState::Running {
        direction: ScpDirection::Upload,
        label: "f".to_string(),
        progress: 0,
        started_at: std::time::Instant::now(),
        file_size: 0,
    };
    app.poll_scp_events();
    // After disconnected rx, scp_rx should be cleared.
    assert!(app.scp_rx.is_none());
}

#[test]
fn poll_scp_events_progress_updates_state() {
    let (tx, rx) = std::sync::mpsc::channel::<crate::ssh::sftp::ScpEvent>();
    let mut app = app_in_running(ScpDirection::Upload);
    app.scp_rx = Some(rx);

    tx.send(crate::ssh::sftp::ScpEvent::Progress(42)).unwrap();
    drop(tx);

    app.poll_scp_events();

    if let ScpState::Running { progress, .. } = &app.scp_state {
        assert_eq!(*progress, 42);
    } else {
        panic!("Expected Running state");
    }
}

#[test]
fn poll_scp_events_file_size_updates_state() {
    let (tx, rx) = std::sync::mpsc::channel::<crate::ssh::sftp::ScpEvent>();
    let mut app = app_in_running(ScpDirection::Download);
    app.scp_rx = Some(rx);

    tx.send(crate::ssh::sftp::ScpEvent::FileSize(1024 * 1024))
        .unwrap();
    drop(tx);

    app.poll_scp_events();

    if let ScpState::Running { file_size, .. } = &app.scp_state {
        assert_eq!(*file_size, 1024 * 1024);
    } else {
        panic!("Expected Running state");
    }
}

#[test]
fn poll_scp_events_done_ok_transitions_to_done() {
    let (tx, rx) = std::sync::mpsc::channel::<crate::ssh::sftp::ScpEvent>();
    let mut app = app_in_running(ScpDirection::Upload);
    app.scp_rx = Some(rx);

    tx.send(crate::ssh::sftp::ScpEvent::Done(true)).unwrap();
    drop(tx);

    app.poll_scp_events();

    assert!(matches!(
        app.scp_state,
        ScpState::Done { exit_ok: true, .. }
    ));
    assert!(app.scp_rx.is_none());
}

#[test]
fn poll_scp_events_done_fail_transitions_to_done() {
    let (tx, rx) = std::sync::mpsc::channel::<crate::ssh::sftp::ScpEvent>();
    let mut app = app_in_running(ScpDirection::Download);
    app.scp_rx = Some(rx);

    tx.send(crate::ssh::sftp::ScpEvent::Done(false)).unwrap();
    drop(tx);

    app.poll_scp_events();

    assert!(matches!(
        app.scp_state,
        ScpState::Done { exit_ok: false, .. }
    ));
}

#[test]
fn poll_scp_events_error_transitions_to_error() {
    let (tx, rx) = std::sync::mpsc::channel::<crate::ssh::sftp::ScpEvent>();
    let mut app = app_in_running(ScpDirection::Upload);
    app.scp_rx = Some(rx);

    tx.send(crate::ssh::sftp::ScpEvent::Error(
        "connection refused".to_string(),
    ))
    .unwrap();
    drop(tx);

    app.poll_scp_events();

    assert!(matches!(app.scp_state, ScpState::Error(_)));
    assert!(app.scp_rx.is_none());
}
