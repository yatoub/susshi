use super::tests_helpers::make_namespace_config;
use super::*;

fn wallix_test_server(group: Option<&str>) -> ResolvedServer {
    ResolvedServer {
        namespace: String::new(),
        group_name: "ALPHA-BD".to_string(),
        env_name: String::new(),
        name: "app-alpha".to_string(),
        host: "APP-ALPHA-BD".to_string(),
        user: "demo_user".to_string(),
        port: 22,
        ssh_key: String::new(),
        ssh_options: vec![],
        default_mode: ConnectionMode::Wallix,
        jump_host: None,
        bastion_host: Some("bastion.example.test".to_string()),
        bastion_user: Some("demo_user".to_string()),
        bastion_template: "{target_user}@%n:SSH:{bastion_user}".to_string(),
        wallix_group: group.map(str::to_string),
        wallix_account: "default".to_string(),
        wallix_protocol: "SSH".to_string(),
        wallix_auto_select: true,
        wallix_fail_if_menu_match_error: true,
        wallix_selection_timeout_secs: 8,
        wallix_direct: false,
        wallix_authorization: None,
        wallix_header_columns: vec![],
        use_system_ssh_config: false,
        probe_filesystems: vec![],
        tunnels: vec![],
        tags: vec![],
        control_master: false,
        agent_forwarding: false,
        control_path: String::new(),
        control_persist: "10m".to_string(),
        pre_connect_hook: None,
        post_disconnect_hook: None,
        hook_timeout_secs: 5,
        ssh_cert: String::new(),
        notes: String::new(),
        production: false,
        confirm_production: false,
        ssh_agent_sock: String::new(),
    }
}

#[test]
fn wallix_selector_required_when_auto_select_disabled() {
    let mut app = App::new(
        make_namespace_config(),
        vec![],
        std::path::PathBuf::new(),
        vec![],
    )
    .unwrap();
    app.connection_mode = ConnectionMode::Wallix;

    let mut server = app.resolved_servers[0].clone();
    server.wallix_auto_select = false;
    server.wallix_fail_if_menu_match_error = true;

    assert!(app.should_open_wallix_selector(&server));
}

#[test]
fn wallix_selector_required_when_auto_select_enabled() {
    let mut app = App::new(
        make_namespace_config(),
        vec![],
        std::path::PathBuf::new(),
        vec![],
    )
    .unwrap();
    app.connection_mode = ConnectionMode::Wallix;

    let mut server = app.resolved_servers[0].clone();
    server.wallix_auto_select = true;
    server.wallix_fail_if_menu_match_error = false;

    assert!(app.should_open_wallix_selector(&server));
}

#[test]
fn wallix_poll_auto_resolves_to_pending_connection() {
    let mut app = App::new(
        make_namespace_config(),
        vec![],
        std::path::PathBuf::new(),
        vec![],
    )
    .unwrap();

    let server = wallix_test_server(Some("dev-admins"));
    let entries = vec![WallixMenuEntry {
        id: "42".to_string(),
        target: "demo_user@default@APP-ALPHA-BD:SSH".to_string(),
        group: "APP-ALPHA_dev-admins".to_string(),
    }];

    let (tx, rx) = std::sync::mpsc::channel();
    app.wallix_selector_rx = Some(rx);
    tx.send((server.clone(), Ok(entries))).unwrap();

    app.poll_wallix_selector();

    assert!(app.wallix_selector.is_none());
    let pending = app.take_pending_wallix_connection();
    assert!(pending.is_some());
    let (_, selected_id, _) = pending.unwrap();
    assert_eq!(selected_id, "42");
    assert_eq!(
        app.wallix_selection_cache.get(&App::server_key(&server)),
        Some(&"42".to_string())
    );
}

#[test]
fn wallix_poll_ambiguous_resolution_opens_targeted_selector() {
    let mut app = App::new(
        make_namespace_config(),
        vec![],
        std::path::PathBuf::new(),
        vec![],
    )
    .unwrap();

    let server = wallix_test_server(Some("dev-admins"));
    let entries = vec![
        WallixMenuEntry {
            id: "11".to_string(),
            target: "demo_user@default@APP-ALPHA-BD:SSH".to_string(),
            group: "APP-ALPHA_dev-admins".to_string(),
        },
        WallixMenuEntry {
            id: "12".to_string(),
            target: "demo_user@default@APP-ALPHA-BD:SSH".to_string(),
            group: "APP-ALPHA_dev-admins".to_string(),
        },
        WallixMenuEntry {
            id: "99".to_string(),
            target: "demo_user@default@OTHER:SSH".to_string(),
            group: "OTHER_dev-admins".to_string(),
        },
    ];

    let (tx, rx) = std::sync::mpsc::channel();
    app.wallix_selector_rx = Some(rx);
    tx.send((server, Ok(entries))).unwrap();

    app.poll_wallix_selector();

    assert!(app.take_pending_wallix_connection().is_none());
    match &app.wallix_selector {
        Some(WallixSelectorState::List { entries, .. }) => {
            assert_eq!(entries.len(), 2);
            assert!(
                entries
                    .iter()
                    .all(|entry| entry.target == "demo_user@default@APP-ALPHA-BD:SSH")
            );
        }
        _ => panic!("expected Wallix selector list"),
    }
}

#[test]
fn wallix_poll_missing_group_opens_targeted_selector() {
    let mut app = App::new(
        make_namespace_config(),
        vec![],
        std::path::PathBuf::new(),
        vec![],
    )
    .unwrap();

    let server = wallix_test_server(None);
    let entries = vec![
        WallixMenuEntry {
            id: "21".to_string(),
            target: "demo_user@default@APP-ALPHA-BD:SSH".to_string(),
            group: "APP-ALPHA_ops-admins".to_string(),
        },
        WallixMenuEntry {
            id: "22".to_string(),
            target: "demo_user@default@APP-ALPHA-BD:SSH".to_string(),
            group: "APP-ALPHA_dev-admins".to_string(),
        },
    ];

    let (tx, rx) = std::sync::mpsc::channel();
    app.wallix_selector_rx = Some(rx);
    tx.send((server, Ok(entries))).unwrap();

    app.poll_wallix_selector();

    assert!(app.take_pending_wallix_connection().is_none());
    assert!(matches!(
        app.wallix_selector,
        Some(WallixSelectorState::List { .. })
    ));
}

#[test]
fn wallix_poll_uses_cached_selection_when_available() {
    let mut app = App::new(
        make_namespace_config(),
        vec![],
        std::path::PathBuf::new(),
        vec![],
    )
    .unwrap();

    let server = wallix_test_server(Some("dev-admins"));
    app.wallix_selection_cache
        .insert(App::server_key(&server), "77".to_string());

    let entries = vec![
        WallixMenuEntry {
            id: "77".to_string(),
            target: "demo_user@default@APP-ALPHA-BD:SSH".to_string(),
            group: "APP-ALPHA_ops-admins".to_string(),
        },
        WallixMenuEntry {
            id: "78".to_string(),
            target: "demo_user@default@APP-ALPHA-BD:SSH".to_string(),
            group: "APP-ALPHA_dev-admins".to_string(),
        },
    ];

    let (tx, rx) = std::sync::mpsc::channel();
    app.wallix_selector_rx = Some(rx);
    tx.send((server, Ok(entries))).unwrap();

    app.poll_wallix_selector();

    assert!(app.wallix_selector.is_none());
    let pending = app.take_pending_wallix_connection();
    assert!(pending.is_some());
    let (_, selected_id, _) = pending.unwrap();
    assert_eq!(selected_id, "77");
}

#[test]
fn targeted_wallix_entries_keeps_matching_target_when_available() {
    let server = wallix_test_server(Some("dev-admins"));

    let entries = vec![
        WallixMenuEntry {
            id: "1".to_string(),
            target: "demo_user@default@OTHER:SSH".to_string(),
            group: "OTHER_dev-admins".to_string(),
        },
        WallixMenuEntry {
            id: "2".to_string(),
            target: "demo_user@default@APP-ALPHA-BD:SSH".to_string(),
            group: "APP-ALPHA_dev-admins".to_string(),
        },
    ];

    let filtered = wallix_state::targeted_wallix_entries(&server, &entries);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, "2");
}

// ── open_wallix_selector (wallix_direct path) ────────────────────────────────

#[test]
fn open_wallix_selector_direct_sets_pending_connection() {
    let mut app = App::new(
        make_namespace_config(),
        vec![],
        std::path::PathBuf::new(),
        vec![],
    )
    .unwrap();

    let mut server = wallix_test_server(None);
    server.wallix_direct = true;

    app.open_wallix_selector(server.clone(), false);

    assert!(app.wallix_selector.is_none());
    let pending = app.take_pending_wallix_connection();
    assert!(pending.is_some());
    let (_, id, _) = pending.unwrap();
    assert_eq!(id, "WALLIX_DIRECT");
}

// ── poll_wallix_selector error paths ─────────────────────────────────────────

#[test]
fn poll_wallix_selector_error_wallix_direct_sets_pending() {
    let mut app = App::new(
        make_namespace_config(),
        vec![],
        std::path::PathBuf::new(),
        vec![],
    )
    .unwrap();

    let server = wallix_test_server(None);
    let (tx, rx) = std::sync::mpsc::channel();
    app.wallix_selector = Some(WallixSelectorState::Loading {
        server: Box::new(server.clone()),
        verbose: false,
    });
    app.wallix_selector_rx = Some(rx);
    tx.send((server, Err("WALLIX_DIRECT_CONNECTION".to_string())))
        .unwrap();

    app.poll_wallix_selector();

    assert!(app.wallix_selector.is_none());
    assert!(app.take_pending_wallix_connection().is_some());
}

#[test]
fn poll_wallix_selector_error_ssh_auth_required_sets_credential_input() {
    let mut app = App::new(
        make_namespace_config(),
        vec![],
        std::path::PathBuf::new(),
        vec![],
    )
    .unwrap();

    let server = wallix_test_server(None);
    let (tx, rx) = std::sync::mpsc::channel();
    app.wallix_selector = Some(WallixSelectorState::Loading {
        server: Box::new(server.clone()),
        verbose: false,
    });
    app.wallix_selector_rx = Some(rx);
    tx.send((
        server,
        Err("SSH_AUTH_REQUIRED:Enter passphrase for key".to_string()),
    ))
    .unwrap();

    app.poll_wallix_selector();

    assert!(app.wallix_selector.is_none());
    assert!(matches!(
        app.app_mode,
        AppMode::CredentialInput {
            is_passphrase: true,
            ..
        }
    ));
}

#[test]
fn poll_wallix_selector_generic_error_sets_error_state() {
    let mut app = App::new(
        make_namespace_config(),
        vec![],
        std::path::PathBuf::new(),
        vec![],
    )
    .unwrap();

    let server = wallix_test_server(None);
    let (tx, rx) = std::sync::mpsc::channel();
    app.wallix_selector_rx = Some(rx);
    tx.send((server, Err("network timeout".to_string())))
        .unwrap();

    app.poll_wallix_selector();

    assert!(matches!(
        app.wallix_selector,
        Some(WallixSelectorState::Error { .. })
    ));
}

#[test]
fn poll_wallix_selector_no_rx_noop() {
    let mut app = App::new(
        make_namespace_config(),
        vec![],
        std::path::PathBuf::new(),
        vec![],
    )
    .unwrap();
    app.wallix_selector_rx = None;
    app.poll_wallix_selector(); // Should not panic.
    assert!(app.wallix_selector.is_none());
}

// ── close_wallix_selector ────────────────────────────────────────────────────

#[test]
fn close_wallix_selector_clears_state() {
    let mut app = App::new(
        make_namespace_config(),
        vec![],
        std::path::PathBuf::new(),
        vec![],
    )
    .unwrap();

    let server = wallix_test_server(None);
    app.wallix_selector = Some(WallixSelectorState::List {
        server: Box::new(server.clone()),
        entries: vec![],
        selected: 0,
    });
    let (_, rx) = std::sync::mpsc::channel::<WallixMenuLoadResult>();
    app.wallix_selector_rx = Some(rx);

    app.close_wallix_selector();

    assert!(app.wallix_selector.is_none());
    assert!(app.wallix_selector_rx.is_none());
}

// ── remember_wallix_selection ─────────────────────────────────────────────────

#[test]
fn remember_wallix_selection_stores_id() {
    let mut app = App::new(
        make_namespace_config(),
        vec![],
        std::path::PathBuf::new(),
        vec![],
    )
    .unwrap();

    let server = wallix_test_server(None);
    app.remember_wallix_selection(&server, "55");

    let key = App::server_key(&server);
    assert_eq!(
        app.wallix_selection_cache.get(&key),
        Some(&"55".to_string())
    );
}

// ── wallix_selector_next / previous / selected_id ────────────────────────────

fn app_with_list_selector(entries: Vec<WallixMenuEntry>) -> App {
    let mut app = App::new(
        make_namespace_config(),
        vec![],
        std::path::PathBuf::new(),
        vec![],
    )
    .unwrap();
    let server = wallix_test_server(None);
    app.wallix_selector = Some(WallixSelectorState::List {
        server: Box::new(server),
        entries,
        selected: 0,
    });
    app
}

#[test]
fn wallix_selector_next_increments() {
    let entries = vec![
        WallixMenuEntry {
            id: "1".into(),
            target: "T1".into(),
            group: "G1".into(),
        },
        WallixMenuEntry {
            id: "2".into(),
            target: "T2".into(),
            group: "G2".into(),
        },
    ];
    let mut app = app_with_list_selector(entries);
    app.wallix_selector_next();
    if let Some(WallixSelectorState::List { selected, .. }) = &app.wallix_selector {
        assert_eq!(*selected, 1);
    }
}

#[test]
fn wallix_selector_next_clamps_at_last() {
    let entries = vec![WallixMenuEntry {
        id: "1".into(),
        target: "T1".into(),
        group: "G1".into(),
    }];
    let mut app = app_with_list_selector(entries);
    app.wallix_selector_next();
    app.wallix_selector_next();
    if let Some(WallixSelectorState::List { selected, .. }) = &app.wallix_selector {
        assert_eq!(*selected, 0); // clamped at len-1 = 0
    }
}

#[test]
fn wallix_selector_previous_decrements() {
    let entries = vec![
        WallixMenuEntry {
            id: "1".into(),
            target: "T1".into(),
            group: "G1".into(),
        },
        WallixMenuEntry {
            id: "2".into(),
            target: "T2".into(),
            group: "G2".into(),
        },
    ];
    let mut app = app_with_list_selector(entries);
    app.wallix_selector_next(); // → 1
    app.wallix_selector_previous(); // → 0
    if let Some(WallixSelectorState::List { selected, .. }) = &app.wallix_selector {
        assert_eq!(*selected, 0);
    }
}

#[test]
fn wallix_selector_previous_saturates_at_zero() {
    let entries = vec![WallixMenuEntry {
        id: "1".into(),
        target: "T1".into(),
        group: "G1".into(),
    }];
    let mut app = app_with_list_selector(entries);
    app.wallix_selector_previous();
    if let Some(WallixSelectorState::List { selected, .. }) = &app.wallix_selector {
        assert_eq!(*selected, 0);
    }
}

#[test]
fn wallix_selector_selected_id_returns_entry() {
    let entries = vec![WallixMenuEntry {
        id: "42".into(),
        target: "T".into(),
        group: "G".into(),
    }];
    let app = app_with_list_selector(entries);
    let result = app.wallix_selector_selected_id();
    assert!(result.is_some());
    let (_, id) = result.unwrap();
    assert_eq!(id, "42");
}

#[test]
fn wallix_selector_selected_id_none_when_not_list() {
    let app = App::new(
        make_namespace_config(),
        vec![],
        std::path::PathBuf::new(),
        vec![],
    )
    .unwrap();
    assert!(app.wallix_selector_selected_id().is_none());
}

// ── poll_wallix_selector: auto_select=false path ─────────────────────────────

#[test]
fn poll_wallix_selector_auto_select_false_opens_list() {
    let mut app = App::new(
        make_namespace_config(),
        vec![],
        std::path::PathBuf::new(),
        vec![],
    )
    .unwrap();

    let mut server = wallix_test_server(None);
    server.wallix_auto_select = false;

    let entries = vec![WallixMenuEntry {
        id: "10".to_string(),
        target: "demo_user@default@APP-ALPHA-BD:SSH".to_string(),
        group: "APP-ALPHA_ops".to_string(),
    }];

    let (tx, rx) = std::sync::mpsc::channel();
    app.wallix_selector_rx = Some(rx);
    tx.send((server, Ok(entries))).unwrap();

    app.poll_wallix_selector();

    assert!(matches!(
        app.wallix_selector,
        Some(WallixSelectorState::List { .. })
    ));
    assert!(app.take_pending_wallix_connection().is_none());
}
