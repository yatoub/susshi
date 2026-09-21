use susshi::config::{ConnectionMode, ResolvedServer};
use susshi::wallix::{build_expected_target, parse_wallix_menu, select_id_for_server};

fn wallix_server(group: Option<&str>) -> ResolvedServer {
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
fn builds_target_from_resolved_config() {
    let server = wallix_server(Some("APP-ALPHA_ops-admins"));
    assert_eq!(
        build_expected_target(&server),
        "demo_user@default@APP-ALPHA-BD:SSH"
    );
}

#[test]
fn selects_expected_id_from_realistic_menu_fixture() {
    let output = r#"
Warning: Permanently added 'bastion.example.test' (ECDSA) to the list of known hosts.

| ID | Cible (page 1/1)               | Autorisation
|----|--------------------------------|-----------------------
|  0 | demo_user@default@APP-ALPHA-BD:SSH | APP-ALPHA_ops-admins
|  1 | demo_user@default@APP-ALPHA-BD:SSH | APP-ALPHA_dev-admins
Tapez h pour l'aide, ctrl-D pour quitter
 >
"#;

    let entries = parse_wallix_menu(output, &[]).unwrap();
    let mut server = wallix_server(Some("dev-admins"));
    server.group_name = "ALPHA".to_string();
    server.env_name = "PP".to_string();

    assert_eq!(select_id_for_server(&entries, &server).unwrap(), "1");
}

#[test]
fn selects_single_entry_when_group_is_missing_from_server_config() {
    let output = r#"
| ID | Cible (page 1/1)               | Autorisation
|----|--------------------------------|-----------------------
|  0 | demo_user@default@APP-ALPHA-BD:SSH | APP-ALPHA_ops-admins
"#;

    let entries = parse_wallix_menu(output, &[]).unwrap();
    let server = wallix_server(None);
    // Single matching entry → auto-selected even without wallix_group configured.
    let id = select_id_for_server(&entries, &server).unwrap();
    assert_eq!(id, "0");
}

#[test]
fn errors_when_group_is_missing_and_multiple_entries_match() {
    let output = r#"
| ID | Cible (page 1/1)               | Autorisation
|----|--------------------------------|-----------------------
|  0 | demo_user@default@APP-ALPHA-BD:SSH | APP-ALPHA_ops-admins
|  1 | demo_user@default@APP-ALPHA-BD:SSH | APP-ALPHA_dev-admins
"#;

    let entries = parse_wallix_menu(output, &[]).unwrap();
    let server = wallix_server(None);
    let error = select_id_for_server(&entries, &server).unwrap_err();
    assert!(error.to_string().contains("Multiple menu entries"));
    assert!(error.to_string().contains("wallix.group"));
}
