use super::*;
use crate::config::{ConfigEntry, Group, Server, TunnelConfig};
use crate::state::TunnelOverride;

fn make_tunnel_cfg(label: &str, local_port: u16) -> TunnelConfig {
    TunnelConfig {
        local_port,
        remote_host: "127.0.0.1".to_string(),
        remote_port: local_port,
        label: label.to_string(),
    }
}

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
    let mut app = tests_helpers::make_test_app(config);
    // Clear overrides loaded from ~/.susshi_state.json to isolate tests.
    app.tunnel_overrides.clear();
    // Expand the group so the server is visible and selectable.
    app.expanded_items.insert("Group:Grp".to_string());
    app.items_dirty = true;
    app.get_visible_items(); // prime the cache
    app.select(1); // select the server (index 1 after the group header)
    app
}

// ── effective_tunnels ────────────────────────────────────────────────────────

#[test]
fn effective_tunnels_no_tunnels() {
    let app = make_app_with_server();
    let server = app.resolved_servers[0].clone();
    let tunnels = app.effective_tunnels(&server);
    assert!(tunnels.is_empty());
}

#[test]
fn effective_tunnels_with_yaml_tunnels() {
    let mut app = make_app_with_server();
    app.resolved_servers[0].tunnels =
        vec![make_tunnel_cfg("pg", 5432), make_tunnel_cfg("redis", 6379)];
    let server = app.resolved_servers[0].clone();
    let tunnels = app.effective_tunnels(&server);
    assert_eq!(tunnels.len(), 2);
    assert_eq!(tunnels[0].config.label, "pg");
    assert_eq!(tunnels[1].config.label, "redis");
}

#[test]
fn effective_tunnels_user_override_appended() {
    let mut app = make_app_with_server();
    let server = app.resolved_servers[0].clone();
    let key = App::server_key(&server);
    app.tunnel_overrides.push(TunnelOverride {
        server_key: key,
        yaml_index: None,
        config: make_tunnel_cfg("web", 8080),
        hidden: false,
    });
    let tunnels = app.effective_tunnels(&server);
    assert_eq!(tunnels.len(), 1);
    assert_eq!(tunnels[0].config.label, "web");
    assert_eq!(tunnels[0].yaml_index, None);
}

// ── add / update / remove tunnel override ───────────────────────────────────

#[test]
fn add_tunnel_override_increases_count() {
    let mut app = make_app_with_server();
    let server = app.resolved_servers[0].clone();
    assert_eq!(app.effective_tunnels(&server).len(), 0);
    app.add_tunnel_override(&server, make_tunnel_cfg("pg", 5432));
    assert_eq!(app.effective_tunnels(&server).len(), 1);
}

#[test]
fn update_tunnel_override_yaml_creates_override() {
    let mut app = make_app_with_server();
    app.resolved_servers[0].tunnels = vec![make_tunnel_cfg("pg", 5432)];
    let server = app.resolved_servers[0].clone();

    let new_cfg = make_tunnel_cfg("pg-edited", 15432);
    app.update_tunnel_override(&server, Some(0), 0, new_cfg);

    let tunnels = app.effective_tunnels(&server);
    assert_eq!(tunnels.len(), 1);
    assert_eq!(tunnels[0].config.label, "pg-edited");
    assert!(tunnels[0].is_overridden);
}

#[test]
fn update_tunnel_override_yaml_updates_existing() {
    let mut app = make_app_with_server();
    app.resolved_servers[0].tunnels = vec![make_tunnel_cfg("pg", 5432)];
    let server = app.resolved_servers[0].clone();
    let key = App::server_key(&server);

    // Pre-existing override for yaml index 0.
    app.tunnel_overrides.push(TunnelOverride {
        server_key: key,
        yaml_index: Some(0),
        config: make_tunnel_cfg("pg-v1", 10001),
        hidden: false,
    });

    app.update_tunnel_override(&server, Some(0), 0, make_tunnel_cfg("pg-v2", 10002));
    let tunnels = app.effective_tunnels(&server);
    assert_eq!(tunnels[0].config.label, "pg-v2");
    assert_eq!(tunnels[0].config.local_port, 10002);
}

#[test]
fn update_tunnel_override_user_tunnel() {
    let mut app = make_app_with_server();
    let server = app.resolved_servers[0].clone();
    let key = App::server_key(&server);

    app.tunnel_overrides.push(TunnelOverride {
        server_key: key,
        yaml_index: None,
        config: make_tunnel_cfg("web", 8080),
        hidden: false,
    });

    app.update_tunnel_override(&server, None, 0, make_tunnel_cfg("web-v2", 9090));
    let tunnels = app.effective_tunnels(&server);
    assert_eq!(tunnels[0].config.label, "web-v2");
    assert_eq!(tunnels[0].config.local_port, 9090);
}

#[test]
fn remove_tunnel_override_yaml_hides_tunnel() {
    let mut app = make_app_with_server();
    app.resolved_servers[0].tunnels = vec![make_tunnel_cfg("pg", 5432)];
    let server = app.resolved_servers[0].clone();

    app.remove_tunnel_override(&server, Some(0), 0);
    let tunnels = app.effective_tunnels(&server);
    assert!(tunnels.is_empty());
}

#[test]
fn remove_tunnel_override_yaml_creates_hidden_override_if_missing() {
    let mut app = make_app_with_server();
    app.resolved_servers[0].tunnels = vec![make_tunnel_cfg("pg", 5432)];
    let server = app.resolved_servers[0].clone();

    // No existing override — should create one with hidden=true.
    assert!(app.tunnel_overrides.is_empty());
    app.remove_tunnel_override(&server, Some(0), 0);
    assert_eq!(app.tunnel_overrides.len(), 1);
    assert!(app.tunnel_overrides[0].hidden);
}

#[test]
fn remove_tunnel_override_user_tunnel_deleted() {
    let mut app = make_app_with_server();
    let server = app.resolved_servers[0].clone();
    let key = App::server_key(&server);

    app.tunnel_overrides.push(TunnelOverride {
        server_key: key,
        yaml_index: None,
        config: make_tunnel_cfg("web", 8080),
        hidden: false,
    });

    app.remove_tunnel_override(&server, None, 0);
    let tunnels = app.effective_tunnels(&server);
    assert!(tunnels.is_empty());
    // The entry itself should be gone from overrides.
    assert!(app.tunnel_overrides.is_empty());
}

// ── active_tunnel_count ──────────────────────────────────────────────────────

#[test]
fn active_tunnel_count_no_tunnels() {
    let app = make_app_with_server();
    let server = app.resolved_servers[0].clone();
    assert_eq!(app.active_tunnel_count(&server), 0);
}

// ── open / close tunnel overlay ─────────────────────────────────────────────

#[test]
fn open_tunnel_overlay_wallix_mode_no_op() {
    let mut app = make_app_with_server();
    app.connection_mode = ConnectionMode::Wallix;
    app.open_tunnel_overlay();
    assert!(app.tunnel_overlay.is_none());
}

#[test]
fn open_tunnel_overlay_with_server_opens() {
    let mut app = make_app_with_server();
    app.open_tunnel_overlay();
    assert!(app.tunnel_overlay.is_some());
    assert!(matches!(
        app.tunnel_overlay,
        Some(TunnelOverlayState::List { selected: 0 })
    ));
}

#[test]
fn close_tunnel_overlay_sets_none() {
    let mut app = make_app_with_server();
    app.tunnel_overlay = Some(TunnelOverlayState::List { selected: 0 });
    app.close_tunnel_overlay();
    assert!(app.tunnel_overlay.is_none());
}

// ── overlay navigation ───────────────────────────────────────────────────────

#[test]
fn tunnel_overlay_next_increments() {
    let mut app = make_app_with_server();
    let server = app.resolved_servers[0].clone();
    let key = App::server_key(&server);
    // Add one user tunnel so the list has 2 items (tunnel + "+" button).
    app.tunnel_overrides.push(TunnelOverride {
        server_key: key,
        yaml_index: None,
        config: make_tunnel_cfg("web", 8080),
        hidden: false,
    });
    app.tunnel_overlay = Some(TunnelOverlayState::List { selected: 0 });

    app.tunnel_overlay_next();
    assert!(matches!(
        app.tunnel_overlay,
        Some(TunnelOverlayState::List { selected: 1 })
    ));
}

#[test]
fn tunnel_overlay_next_wraps_around() {
    let mut app = make_app_with_server();
    // With no tunnels, count = 0 + 1 = 1 (only the "+" button).
    app.tunnel_overlay = Some(TunnelOverlayState::List { selected: 0 });

    app.tunnel_overlay_next();
    assert!(matches!(
        app.tunnel_overlay,
        Some(TunnelOverlayState::List { selected: 0 })
    ));
}

#[test]
fn tunnel_overlay_previous_from_zero_wraps() {
    let mut app = make_app_with_server();
    let server = app.resolved_servers[0].clone();
    let key = App::server_key(&server);
    app.tunnel_overrides.push(TunnelOverride {
        server_key: key,
        yaml_index: None,
        config: make_tunnel_cfg("web", 8080),
        hidden: false,
    });
    app.tunnel_overlay = Some(TunnelOverlayState::List { selected: 0 });

    app.tunnel_overlay_previous();
    // count = 2, prev of 0 = count - 1 = 1
    assert!(matches!(
        app.tunnel_overlay,
        Some(TunnelOverlayState::List { selected: 1 })
    ));
}

#[test]
fn tunnel_overlay_previous_decrements() {
    let mut app = make_app_with_server();
    let server = app.resolved_servers[0].clone();
    let key = App::server_key(&server);
    app.tunnel_overrides.push(TunnelOverride {
        server_key: key,
        yaml_index: None,
        config: make_tunnel_cfg("web", 8080),
        hidden: false,
    });
    app.tunnel_overlay = Some(TunnelOverlayState::List { selected: 1 });

    app.tunnel_overlay_previous();
    assert!(matches!(
        app.tunnel_overlay,
        Some(TunnelOverlayState::List { selected: 0 })
    ));
}

// ── overlay toggle (+ button is no-op) ───────────────────────────────────────

#[test]
fn tunnel_overlay_toggle_plus_button_noop() {
    let mut app = make_app_with_server();
    // No tunnels → only "+" button at index 0.
    app.tunnel_overlay = Some(TunnelOverlayState::List { selected: 0 });
    // Should not panic.
    app.tunnel_overlay_toggle();
}

// ── overlay delete ───────────────────────────────────────────────────────────

#[test]
fn tunnel_overlay_delete_plus_button_noop() {
    let mut app = make_app_with_server();
    // No tunnels → only "+" button at index 0.
    app.tunnel_overlay = Some(TunnelOverlayState::List { selected: 0 });
    app.tunnel_overlay_delete();
    // Should not change anything meaningful.
    assert!(app.tunnel_overlay.is_some());
}

#[test]
fn tunnel_overlay_delete_removes_user_tunnel() {
    let mut app = make_app_with_server();
    let server = app.resolved_servers[0].clone();
    let key = App::server_key(&server);
    app.tunnel_overrides.push(TunnelOverride {
        server_key: key,
        yaml_index: None,
        config: make_tunnel_cfg("web", 8080),
        hidden: false,
    });
    app.tunnel_overlay = Some(TunnelOverlayState::List { selected: 0 });

    app.tunnel_overlay_delete();
    let server = app.resolved_servers[0].clone();
    assert_eq!(app.effective_tunnels(&server).len(), 0);
}

// ── form open (edit / add) ───────────────────────────────────────────────────

#[test]
fn open_tunnel_form_edit_plus_button_noop() {
    let mut app = make_app_with_server();
    // No tunnels → selected=0 is "+" button.
    app.tunnel_overlay = Some(TunnelOverlayState::List { selected: 0 });
    app.open_tunnel_form_edit();
    assert!(matches!(
        app.tunnel_overlay,
        Some(TunnelOverlayState::List { .. })
    ));
}

#[test]
fn open_tunnel_form_edit_opens_form() {
    let mut app = make_app_with_server();
    let server = app.resolved_servers[0].clone();
    let key = App::server_key(&server);
    app.tunnel_overrides.push(TunnelOverride {
        server_key: key,
        yaml_index: None,
        config: make_tunnel_cfg("web", 8080),
        hidden: false,
    });
    app.tunnel_overlay = Some(TunnelOverlayState::List { selected: 0 });

    app.open_tunnel_form_edit();
    assert!(matches!(
        app.tunnel_overlay,
        Some(TunnelOverlayState::Form(_))
    ));
}

#[test]
fn open_tunnel_form_add_opens_form() {
    let mut app = make_app_with_server();
    app.tunnel_overlay = Some(TunnelOverlayState::List { selected: 0 });
    app.open_tunnel_form_add();
    assert!(matches!(
        app.tunnel_overlay,
        Some(TunnelOverlayState::Form(_))
    ));
}

#[test]
fn open_tunnel_form_add_noop_when_no_overlay() {
    let mut app = make_app_with_server();
    // No overlay at all → should be a no-op.
    app.tunnel_overlay = None;
    app.open_tunnel_form_add();
    assert!(app.tunnel_overlay.is_none());
}

// ── form char / backspace / field navigation ─────────────────────────────────

fn app_with_form() -> App {
    let mut app = make_app_with_server();
    app.tunnel_overlay = Some(TunnelOverlayState::Form(TunnelForm::new_empty()));
    app
}

#[test]
fn tunnel_form_char_appends_to_label() {
    let mut app = app_with_form();
    app.tunnel_form_char('a');
    app.tunnel_form_char('b');
    if let Some(TunnelOverlayState::Form(f)) = &app.tunnel_overlay {
        assert_eq!(f.label, "ab");
    } else {
        panic!("Expected Form state");
    }
}

#[test]
fn tunnel_form_char_port_field_rejects_non_digit() {
    let mut app = app_with_form();
    app.tunnel_form_next_field(); // move to LocalPort
    app.tunnel_form_char('5');
    app.tunnel_form_char('x'); // should be rejected
    if let Some(TunnelOverlayState::Form(f)) = &app.tunnel_overlay {
        assert_eq!(f.local_port, "5");
    } else {
        panic!("Expected Form state");
    }
}

#[test]
fn tunnel_form_backspace_removes_last_char() {
    let mut app = app_with_form();
    app.tunnel_form_char('a');
    app.tunnel_form_char('b');
    app.tunnel_form_backspace();
    if let Some(TunnelOverlayState::Form(f)) = &app.tunnel_overlay {
        assert_eq!(f.label, "a");
    } else {
        panic!("Expected Form state");
    }
}

#[test]
fn tunnel_form_next_field_cycles() {
    let mut app = app_with_form();
    if let Some(TunnelOverlayState::Form(f)) = &app.tunnel_overlay {
        assert_eq!(f.focus, TunnelFormField::Label);
    }
    app.tunnel_form_next_field();
    if let Some(TunnelOverlayState::Form(f)) = &app.tunnel_overlay {
        assert_eq!(f.focus, TunnelFormField::LocalPort);
    }
}

#[test]
fn tunnel_form_prev_field_cycles_back() {
    let mut app = app_with_form();
    app.tunnel_form_prev_field();
    if let Some(TunnelOverlayState::Form(f)) = &app.tunnel_overlay {
        assert_eq!(f.focus, TunnelFormField::RemotePort);
    }
}

// ── form submit ───────────────────────────────────────────────────────────────

#[test]
fn tunnel_form_submit_invalid_stores_error() {
    let mut app = app_with_form();
    // Empty form → validation fails.
    app.tunnel_form_submit();
    if let Some(TunnelOverlayState::Form(f)) = &app.tunnel_overlay {
        assert!(!f.error.is_empty());
    } else {
        panic!("Expected Form to stay open with error");
    }
}

#[test]
fn tunnel_form_submit_valid_creates_tunnel_and_goes_to_list() {
    let mut app = app_with_form();
    // Fill a valid form.
    if let Some(TunnelOverlayState::Form(f)) = &mut app.tunnel_overlay {
        f.label = "pg".to_string();
        f.local_port = "5432".to_string();
        f.remote_host = "127.0.0.1".to_string();
        f.remote_port = "5432".to_string();
    }
    app.tunnel_form_submit();
    // Should return to list view.
    assert!(matches!(
        app.tunnel_overlay,
        Some(TunnelOverlayState::List { .. })
    ));
    // And the tunnel should be added.
    let server = app.resolved_servers[0].clone();
    assert_eq!(app.effective_tunnels(&server).len(), 1);
}

// ── form cancel ──────────────────────────────────────────────────────────────

#[test]
fn tunnel_form_cancel_returns_to_list() {
    let mut app = app_with_form();
    app.tunnel_form_cancel();
    assert!(matches!(
        app.tunnel_overlay,
        Some(TunnelOverlayState::List { .. })
    ));
}

// ── stop_all_tunnels / poll_tunnel_events ────────────────────────────────────

#[test]
fn stop_all_tunnels_no_tunnels_noop() {
    let mut app = make_app_with_server();
    // Should not panic with no active tunnels.
    app.stop_all_tunnels();
}

#[test]
fn poll_tunnel_events_no_tunnels_noop() {
    let mut app = make_app_with_server();
    // Should not panic.
    app.poll_tunnel_events();
}
