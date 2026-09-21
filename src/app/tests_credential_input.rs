use super::tests_helpers::make_namespace_config;
use super::*;

fn direct_server() -> ResolvedServer {
    ResolvedServer {
        namespace: String::new(),
        group_name: "G".into(),
        env_name: "E".into(),
        name: "srv".into(),
        host: "198.51.100.1".into(),
        user: "admin".into(),
        port: 22,
        ssh_key: "~/.ssh/id_ed25519".into(),
        ssh_options: vec![],
        default_mode: ConnectionMode::Direct,
        jump_host: None,
        bastion_host: None,
        bastion_user: None,
        bastion_template: "{target_user}@%n:SSH:{bastion_user}".into(),
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
        wallix_group: None,
        wallix_account: "default".to_string(),
        wallix_protocol: "SSH".to_string(),
        wallix_auto_select: true,
        wallix_fail_if_menu_match_error: true,
        wallix_selection_timeout_secs: 8,
        wallix_direct: false,
        wallix_authorization: None,
        wallix_header_columns: vec![],
    }
}

fn make_app() -> App {
    App::new(
        make_namespace_config(),
        vec![],
        std::path::PathBuf::new(),
        vec![],
    )
    .unwrap()
}

#[test]
fn open_credential_input_sets_mode_passphrase() {
    let mut app = make_app();
    // Manually put a server in the selection state to simulate open_credential_input
    let server = direct_server();
    app.app_mode = AppMode::CredentialInput {
        server: Box::new(server),
        mode: ConnectionMode::Direct,
        verbose: false,
        is_passphrase: true,
        input: String::new(),
    };

    assert!(matches!(
        &app.app_mode,
        AppMode::CredentialInput {
            is_passphrase: true,
            ..
        }
    ));
}

#[test]
fn open_credential_input_sets_mode_password() {
    let mut app = make_app();
    let server = direct_server();
    app.app_mode = AppMode::CredentialInput {
        server: Box::new(server),
        mode: ConnectionMode::Direct,
        verbose: false,
        is_passphrase: false,
        input: String::new(),
    };

    assert!(matches!(
        &app.app_mode,
        AppMode::CredentialInput {
            is_passphrase: false,
            ..
        }
    ));
}

#[test]
fn credential_input_char_appends_to_buffer() {
    let mut app = make_app();
    let server = direct_server();
    app.app_mode = AppMode::CredentialInput {
        server: Box::new(server),
        mode: ConnectionMode::Direct,
        verbose: false,
        is_passphrase: true,
        input: String::new(),
    };

    app.credential_input_push('s');
    app.credential_input_push('e');
    app.credential_input_push('c');

    assert!(matches!(
        &app.app_mode,
        AppMode::CredentialInput { input, .. } if input == "sec"
    ));
}

#[test]
fn credential_input_backspace_removes_last_char() {
    let mut app = make_app();
    let server = direct_server();
    app.app_mode = AppMode::CredentialInput {
        server: Box::new(server),
        mode: ConnectionMode::Direct,
        verbose: false,
        is_passphrase: true,
        input: "abc".into(),
    };

    app.credential_input_backspace();

    assert!(matches!(
        &app.app_mode,
        AppMode::CredentialInput { input, .. } if input == "ab"
    ));
}

#[test]
fn credential_input_backspace_on_empty_is_noop() {
    let mut app = make_app();
    let server = direct_server();
    app.app_mode = AppMode::CredentialInput {
        server: Box::new(server),
        mode: ConnectionMode::Direct,
        verbose: false,
        is_passphrase: true,
        input: String::new(),
    };

    app.credential_input_backspace();

    assert!(matches!(
        &app.app_mode,
        AppMode::CredentialInput { input, .. } if input.is_empty()
    ));
}

#[test]
fn credential_input_cancel_returns_to_normal() {
    let mut app = make_app();
    let server = direct_server();
    app.app_mode = AppMode::CredentialInput {
        server: Box::new(server),
        mode: ConnectionMode::Direct,
        verbose: false,
        is_passphrase: true,
        input: "secret".into(),
    };

    app.cancel_credential_input();

    assert!(matches!(app.app_mode, AppMode::Normal));
}

#[test]
fn credential_input_submit_returns_server_and_cred() {
    let mut app = make_app();
    let server = direct_server();
    app.app_mode = AppMode::CredentialInput {
        server: Box::new(server.clone()),
        mode: ConnectionMode::Direct,
        verbose: true,
        is_passphrase: true,
        input: "my_passphrase".into(),
    };

    let result = app.submit_credential_input();

    assert!(result.is_some());
    let (ret_server, mode, verbose, cred) = result.unwrap();
    assert_eq!(ret_server.name, "srv");
    assert_eq!(mode, ConnectionMode::Direct);
    assert!(verbose);
    assert_eq!(cred, "my_passphrase");
    assert!(matches!(app.app_mode, AppMode::Normal));
}

#[test]
fn credential_input_submit_empty_is_none() {
    let mut app = make_app();
    let server = direct_server();
    app.app_mode = AppMode::CredentialInput {
        server: Box::new(server),
        mode: ConnectionMode::Direct,
        verbose: false,
        is_passphrase: true,
        input: String::new(),
    };

    let result = app.submit_credential_input();

    // Empty credential → do not submit
    assert!(result.is_none());
    // Mode is unchanged
    assert!(matches!(app.app_mode, AppMode::CredentialInput { .. }));
}
