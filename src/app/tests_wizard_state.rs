use super::*;
use std::fs;

fn make_empty_config() -> Config {
    Config {
        defaults: None,
        includes: vec![],
        groups: vec![],
        vars: Default::default(),
    }
}

fn make_wizard_app() -> (tempfile::TempDir, App) {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("susshi.yml");
    fs::write(&config_path, "groups: []\n").unwrap();
    let app = App::new(make_empty_config(), vec![], config_path, vec![]).unwrap();
    (temp, app)
}

// ── render_group_yaml (pure, no I/O) ──────────────────────────────────────────

#[test]
fn render_group_yaml_includes_all_fields_when_user_set() {
    let form = WizardForm {
        group_name: "My Servers".to_string(),
        server_name: "web-01".to_string(),
        host: "10.0.0.5".to_string(),
        user: "deploy".to_string(),
    };
    let yaml = render_group_yaml(&form);
    let parsed: Config = serde_yaml_ng::from_str(&yaml).unwrap();
    assert_eq!(parsed.groups.len(), 1);
    let ConfigEntry::Group(group) = &parsed.groups[0] else {
        panic!("expected a Group entry");
    };
    assert_eq!(group.name, "My Servers");
    let servers = group.servers.as_ref().unwrap();
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].name, "web-01");
    assert_eq!(servers[0].host, "10.0.0.5");
    assert_eq!(servers[0].user.as_deref(), Some("deploy"));
}

#[test]
fn render_group_yaml_omits_user_when_blank() {
    let form = WizardForm {
        group_name: "Grp".to_string(),
        server_name: "srv".to_string(),
        host: "1.2.3.4".to_string(),
        user: String::new(),
    };
    let yaml = render_group_yaml(&form);
    let parsed: Config = serde_yaml_ng::from_str(&yaml).unwrap();
    let ConfigEntry::Group(group) = &parsed.groups[0] else {
        panic!("expected a Group entry");
    };
    assert!(group.servers.as_ref().unwrap()[0].user.is_none());
}

#[test]
fn render_group_yaml_escapes_quotes_and_backslashes() {
    let form = WizardForm {
        group_name: "Weird \"Name\"".to_string(),
        server_name: "srv".to_string(),
        host: "1.2.3.4".to_string(),
        user: String::new(),
    };
    let yaml = render_group_yaml(&form);
    let parsed: Config = serde_yaml_ng::from_str(&yaml).unwrap();
    let ConfigEntry::Group(group) = &parsed.groups[0] else {
        panic!("expected a Group entry");
    };
    assert_eq!(group.name, "Weird \"Name\"");
}

// ── wizard field navigation and text editing ──────────────────────────────────

#[test]
fn start_wizard_opens_form_focused_on_group_name() {
    let (_temp, mut app) = make_wizard_app();
    app.start_wizard();
    match &app.wizard_state {
        WizardState::Form { focus, form, .. } => {
            assert_eq!(*focus, WizardField::GroupName);
            assert_eq!(form.group_name, "");
        }
        WizardState::Idle => panic!("expected Form state"),
    }
}

#[test]
fn wizard_push_char_and_backspace_edit_focused_field() {
    let (_temp, mut app) = make_wizard_app();
    app.start_wizard();
    app.wizard_push_char('a');
    app.wizard_push_char('b');
    let WizardState::Form { form, .. } = &app.wizard_state else {
        panic!("expected Form state");
    };
    assert_eq!(form.group_name, "ab");

    app.wizard_backspace();
    let WizardState::Form { form, .. } = &app.wizard_state else {
        panic!("expected Form state");
    };
    assert_eq!(form.group_name, "a");
}

#[test]
fn wizard_next_field_cycles_through_all_fields_and_back() {
    let (_temp, mut app) = make_wizard_app();
    app.start_wizard();
    assert_eq!(current_focus(&app), WizardField::GroupName);
    app.wizard_next_field();
    assert_eq!(current_focus(&app), WizardField::ServerName);
    app.wizard_next_field();
    assert_eq!(current_focus(&app), WizardField::Host);
    app.wizard_next_field();
    assert_eq!(current_focus(&app), WizardField::User);
    app.wizard_next_field();
    assert_eq!(current_focus(&app), WizardField::GroupName);
}

#[test]
fn wizard_prev_field_cycles_backwards() {
    let (_temp, mut app) = make_wizard_app();
    app.start_wizard();
    app.wizard_prev_field();
    assert_eq!(current_focus(&app), WizardField::User);
}

#[test]
fn wizard_push_char_edits_the_field_currently_focused() {
    let (_temp, mut app) = make_wizard_app();
    app.start_wizard();
    app.wizard_next_field(); // ServerName
    app.wizard_push_char('x');
    let WizardState::Form { form, .. } = &app.wizard_state else {
        panic!("expected Form state");
    };
    assert_eq!(form.server_name, "x");
    assert_eq!(form.group_name, "");
}

fn current_focus(app: &App) -> WizardField {
    match &app.wizard_state {
        WizardState::Form { focus, .. } => *focus,
        WizardState::Idle => panic!("expected Form state"),
    }
}

// ── wizard_cancel ──────────────────────────────────────────────────────────────

#[test]
fn wizard_cancel_returns_to_idle_without_touching_the_config_file() {
    let (_temp, mut app) = make_wizard_app();
    let config_path = app.config_path.clone();
    app.start_wizard();
    app.wizard_push_char('a');
    app.wizard_cancel();
    assert!(matches!(app.wizard_state, WizardState::Idle));
    assert_eq!(fs::read_to_string(&config_path).unwrap(), "groups: []\n");
}

// ── wizard_submit: validation ──────────────────────────────────────────────────

#[test]
fn wizard_submit_rejects_empty_group_name() {
    let (_temp, mut app) = make_wizard_app();
    app.start_wizard();
    app.wizard_next_field(); // ServerName
    app.wizard_push_char('s');
    app.wizard_next_field(); // Host
    app.wizard_push_char('h');
    app.wizard_submit().unwrap();
    let WizardState::Form { error, .. } = &app.wizard_state else {
        panic!("expected Form state to remain open on validation error");
    };
    assert!(error.is_some());
}

#[test]
fn wizard_submit_rejects_empty_server_name() {
    let (_temp, mut app) = make_wizard_app();
    app.start_wizard();
    app.wizard_push_char('g');
    app.wizard_next_field(); // ServerName (left blank)
    app.wizard_next_field(); // Host
    app.wizard_push_char('h');
    app.wizard_submit().unwrap();
    assert!(matches!(&app.wizard_state, WizardState::Form { .. }));
}

#[test]
fn wizard_submit_rejects_empty_host() {
    let (_temp, mut app) = make_wizard_app();
    app.start_wizard();
    app.wizard_push_char('g');
    app.wizard_next_field();
    app.wizard_push_char('s');
    app.wizard_submit().unwrap();
    assert!(matches!(&app.wizard_state, WizardState::Form { .. }));
}

// ── wizard_submit: success writes the file, reloads, closes the wizard ─────────

#[test]
fn wizard_submit_writes_config_reloads_and_closes_wizard() {
    let (_temp, mut app) = make_wizard_app();
    app.start_wizard();
    for c in "My Group".chars() {
        app.wizard_push_char(c);
    }
    app.wizard_next_field();
    for c in "web-01".chars() {
        app.wizard_push_char(c);
    }
    app.wizard_next_field();
    for c in "10.0.0.9".chars() {
        app.wizard_push_char(c);
    }
    app.wizard_next_field();
    for c in "deploy".chars() {
        app.wizard_push_char(c);
    }

    app.wizard_submit().unwrap();

    assert!(matches!(app.wizard_state, WizardState::Idle));
    assert_eq!(app.resolved_servers.len(), 1);
    assert_eq!(app.resolved_servers[0].name, "web-01");
    assert_eq!(app.resolved_servers[0].host, "10.0.0.9");
    assert_eq!(app.resolved_servers[0].user, "deploy");

    let on_disk = fs::read_to_string(&app.config_path).unwrap();
    let reparsed: Config = serde_yaml_ng::from_str(&on_disk).unwrap();
    assert_eq!(reparsed.groups.len(), 1);
}
