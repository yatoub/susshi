use super::*;

#[test]
fn test_strip_ansi_removes_csi_sequences() {
    // CSI clear screen + cursor home
    assert_eq!(
        strip_ansi("\x1b[2J\x1b[H| 0 | target | group |"),
        "| 0 | target | group |"
    );
}

#[test]
fn test_strip_ansi_removes_color_codes() {
    assert_eq!(strip_ansi("\x1b[1;32mhello\x1b[0m"), "hello");
}

#[test]
fn test_strip_ansi_passthrough_plain_text() {
    let s = "| 42 | pcollin@default@HOST:SSH | GROUP_ces3s-admins |";
    assert_eq!(strip_ansi(s), s);
}

#[test]
fn test_parse_wallix_menu_with_ansi_codes() {
    let output = "\x1b[2J\x1b[H| ID | Cible (page 1/1)   | Autorisation\n\
                  |----|--------------------|-----------\n\
                  |  0 | demo@default@HOST:SSH | STI-GROUP_ces3s-admins |\n\
                  \x1b[1m > \x1b[0m";
    let entries = parse_wallix_menu(output, &[]).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, "0");
    assert_eq!(entries[0].group, "STI-GROUP_ces3s-admins");
}

#[test]
fn test_group_suffix_matches_short_group() {
    assert!(group_suffix_matches("APP-ANSCORE_dev-admins", "dev-admins"));
}

#[test]
fn test_group_suffix_matches_exact_group() {
    assert!(group_suffix_matches("dev-admins", "dev-admins"));
}

#[test]
fn test_group_suffix_does_not_match_unrelated_group() {
    assert!(!group_suffix_matches(
        "APP-ANSCORE_ops-admins",
        "dev-admins"
    ));
}

#[test]
fn test_group_suffix_matches_case_insensitive() {
    assert!(group_suffix_matches("APP-ANSCORE_DEV-ADMINS", "dev-admins"));
    assert!(group_suffix_matches("Dev-Admins", "dev-admins"));
}

#[test]
fn test_parse_simple_menu() {
    let output = r#"
   ID │ Cible                              │ Autorisation
───────┼────────────────────────────────────┼──────────────────────
 1234  │ demo_user@default@APP-ALPHA-BD:SSH    │ APP-ALPHA_ops-admins
 5678  │ demo_user@default@APP-ALPHA-BD:SSH    │ APP-ALPHA_dev-admins
"#;
    let entries = parse_wallix_menu(output, &[]).expect("Should parse menu");
    assert_eq!(entries.len(), 2);

    assert_eq!(entries[0].id, "1234");
    assert_eq!(entries[0].target, "demo_user@default@APP-ALPHA-BD:SSH");
    assert_eq!(entries[0].group, "APP-ALPHA_ops-admins");

    assert_eq!(entries[1].id, "5678");
    assert_eq!(entries[1].group, "APP-ALPHA_dev-admins");
}

#[test]
fn test_parse_menu_with_varied_whitespace() {
    let output = "  123   │   user@default@host:SSH   │   group-name  ";
    let entries = parse_wallix_menu(output, &[]).expect("Should parse despite whitespace");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, "123");
    assert_eq!(entries[0].target, "user@default@host:SSH");
    assert_eq!(entries[0].group, "group-name");
}

#[test]
fn test_parse_menu_with_leading_zeros() {
    let output = "0001  │ user@default@host:SSH  │ my-group";
    let entries = parse_wallix_menu(output, &[]).expect("Should preserve leading zeros");
    assert_eq!(entries[0].id, "0001");
}

#[test]
fn test_parse_with_custom_header_columns() {
    let output = r#"
   Num │ Target                              │ Authorization
───────┼─────────────────────────────────────┼──────────────────────
  1234 │ ops@default@APP-01:SSH              │ OPS-admins
"#;
    // Default headers ("ID", "Cible", "Autorisation") don't match → would include header as data.
    // Custom headers correctly skip the header line.
    let custom = vec![
        "Num".to_string(),
        "Target".to_string(),
        "Authorization".to_string(),
    ];
    let entries = parse_wallix_menu(output, &custom).expect("devrait parser avec headers custom");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, "1234");
    assert_eq!(entries[0].group, "OPS-admins");
}

#[test]
fn test_parse_empty_output_returns_error() {
    let output = r#"
   ID │ Cible │ Autorisation
───────┼───────┼──────────────
"#;
    let result = parse_wallix_menu(output, &[]);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("No valid menu entries")
    );
}

#[test]
fn test_select_unique_target_and_group_match() {
    let entries = vec![
        WallixMenuEntry {
            id: "1234".to_string(),
            target: "user@default@host:SSH".to_string(),
            group: "admins".to_string(),
        },
        WallixMenuEntry {
            id: "5678".to_string(),
            target: "user@default@other:SSH".to_string(),
            group: "admins".to_string(),
        },
    ];

    let id = select_id_by_target_and_group(&entries, "user@default@host:SSH", "admins")
        .expect("Should find unique match");
    assert_eq!(id, "1234");
}

#[test]
fn test_select_no_target_match() {
    let entries = vec![WallixMenuEntry {
        id: "1234".to_string(),
        target: "user@default@host:SSH".to_string(),
        group: "admins".to_string(),
    }];

    let result = select_id_by_target_and_group(&entries, "other@default@host:SSH", "admins");
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("No menu entry found with target")
    );
}

#[test]
fn test_select_target_match_but_no_group_match() {
    let entries = vec![
        WallixMenuEntry {
            id: "1234".to_string(),
            target: "user@default@host:SSH".to_string(),
            group: "admins".to_string(),
        },
        WallixMenuEntry {
            id: "5678".to_string(),
            target: "user@default@host:SSH".to_string(),
            group: "operators".to_string(),
        },
    ];

    let result = select_id_by_target_and_group(&entries, "user@default@host:SSH", "users");
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("No menu entry found for target")
    );
}

#[test]
fn test_select_multiple_matches_returns_error() {
    let entries = vec![
        WallixMenuEntry {
            id: "1234".to_string(),
            target: "user@default@host:SSH".to_string(),
            group: "admins".to_string(),
        },
        WallixMenuEntry {
            id: "5678".to_string(),
            target: "user@default@host:SSH".to_string(),
            group: "admins".to_string(),
        },
    ];

    let result = select_id_by_target_and_group(&entries, "user@default@host:SSH", "admins");
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Multiple menu entries")
    );
}

#[test]
fn test_build_expected_target_from_resolved_server() {
    let server = ResolvedServer {
        namespace: String::new(),
        group_name: String::new(),
        env_name: String::new(),
        name: "alpha-ops".to_string(),
        host: "APP-ALPHA-BD".to_string(),
        user: "demo_user".to_string(),
        port: 22,
        ssh_key: String::new(),
        ssh_options: vec![],
        default_mode: crate::config::ConnectionMode::Wallix,
        jump_host: None,
        bastion_host: Some("bastion.example.test".to_string()),
        bastion_user: Some("demo_user".to_string()),
        bastion_template: "{target_user}@%n:SSH:{bastion_user}".to_string(),
        wallix_group: Some("APP-ALPHA_ops-admins".to_string()),
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
        ssh_agent_sock: String::new(),
    };

    assert_eq!(
        build_expected_target(&server),
        "demo_user@default@APP-ALPHA-BD:SSH"
    );
}

#[test]
fn test_select_id_for_server_uses_resolved_server_fields() {
    let entries = vec![WallixMenuEntry {
        id: "0".to_string(),
        target: "demo_user@default@APP-ALPHA-BD:SSH".to_string(),
        group: "APP-ALPHA_ops-admins".to_string(),
    }];

    let server = ResolvedServer {
        namespace: String::new(),
        group_name: String::new(),
        env_name: String::new(),
        name: "alpha-ops".to_string(),
        host: "APP-ALPHA-BD".to_string(),
        user: "demo_user".to_string(),
        port: 22,
        ssh_key: String::new(),
        ssh_options: vec![],
        default_mode: crate::config::ConnectionMode::Wallix,
        jump_host: None,
        bastion_host: Some("bastion.example.test".to_string()),
        bastion_user: Some("demo_user".to_string()),
        bastion_template: "{target_user}@%n:SSH:{bastion_user}".to_string(),
        wallix_group: Some("APP-ALPHA_ops-admins".to_string()),
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
        ssh_agent_sock: String::new(),
    };

    assert_eq!(select_id_for_server(&entries, &server).unwrap(), "0");
}

#[test]
fn test_select_id_for_server_single_entry_no_group() {
    let entries = vec![WallixMenuEntry {
        id: "0".to_string(),
        target: "demo_user@default@APP-ALPHA-BD:SSH".to_string(),
        group: "APP-ALPHA_ops-admins".to_string(),
    }];

    let server = ResolvedServer {
        namespace: String::new(),
        group_name: String::new(),
        env_name: String::new(),
        name: "app-alpha-missing-group".to_string(),
        host: "APP-ALPHA-BD".to_string(),
        user: "demo_user".to_string(),
        port: 22,
        ssh_key: String::new(),
        ssh_options: vec![],
        default_mode: crate::config::ConnectionMode::Wallix,
        jump_host: None,
        bastion_host: Some("bastion.example.test".to_string()),
        bastion_user: Some("demo_user".to_string()),
        bastion_template: "{target_user}@%n:SSH:{bastion_user}".to_string(),
        wallix_group: None,
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
        ssh_agent_sock: String::new(),
    };

    // Single entry matching the target → auto-selected even without wallix_group.
    let id = select_id_for_server(&entries, &server).unwrap();
    assert_eq!(id, "0");
}

#[test]
fn test_select_id_for_server_multi_entry_no_group_returns_error() {
    let entries = vec![
        WallixMenuEntry {
            id: "0".to_string(),
            target: "demo_user@default@APP-ALPHA-BD:SSH".to_string(),
            group: "APP-ALPHA_ops-admins".to_string(),
        },
        WallixMenuEntry {
            id: "1".to_string(),
            target: "demo_user@default@APP-ALPHA-BD:SSH".to_string(),
            group: "APP-ALPHA_dev-admins".to_string(),
        },
    ];

    let server = ResolvedServer {
        namespace: String::new(),
        group_name: String::new(),
        env_name: String::new(),
        name: "app-alpha-ambiguous".to_string(),
        host: "APP-ALPHA-BD".to_string(),
        user: "demo_user".to_string(),
        port: 22,
        ssh_key: String::new(),
        ssh_options: vec![],
        default_mode: crate::config::ConnectionMode::Wallix,
        jump_host: None,
        bastion_host: Some("bastion.example.test".to_string()),
        bastion_user: Some("demo_user".to_string()),
        bastion_template: "{target_user}@%n:SSH:{bastion_user}".to_string(),
        wallix_group: None,
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
        ssh_agent_sock: String::new(),
    };

    // Multiple entries with no group configured → error prompting user to set wallix.group.
    let error = select_id_for_server(&entries, &server).unwrap_err();
    assert!(error.to_string().contains("Multiple menu entries"));
    assert!(error.to_string().contains("wallix.group"));
}

#[test]
fn test_build_expected_groups_adds_yaml_structure_prefix() {
    let server = ResolvedServer {
        namespace: String::new(),
        group_name: "ALPHA".to_string(),
        env_name: "PP".to_string(),
        name: "alpha-dev".to_string(),
        host: "APP-ALPHA-BD".to_string(),
        user: "demo_user".to_string(),
        port: 22,
        ssh_key: String::new(),
        ssh_options: vec![],
        default_mode: crate::config::ConnectionMode::Wallix,
        jump_host: None,
        bastion_host: Some("bastion.example.test".to_string()),
        bastion_user: Some("demo_user".to_string()),
        bastion_template: "{target_user}@%n:SSH:{bastion_user}".to_string(),
        wallix_group: Some("dev-admins".to_string()),
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
        ssh_agent_sock: String::new(),
    };

    let groups = build_expected_groups(&server).unwrap();
    assert_eq!(
        groups,
        vec!["dev-admins".to_string(), "PP-ALPHA_dev-admins".to_string()]
    );
}

#[test]
fn test_build_expected_targets_adds_wallix_alias_from_fqdn_and_yaml() {
    let server = ResolvedServer {
        namespace: String::new(),
        group_name: "ALPHA".to_string(),
        env_name: "PP".to_string(),
        name: "bdd01".to_string(),
        host: "app-db01.alpha.example.test".to_string(),
        user: "demo_user".to_string(),
        port: 22,
        ssh_key: String::new(),
        ssh_options: vec![],
        default_mode: crate::config::ConnectionMode::Wallix,
        jump_host: None,
        bastion_host: Some("bastion.example.test".to_string()),
        bastion_user: Some("demo_user".to_string()),
        bastion_template: "{target_user}@%n:SSH:{bastion_user}".to_string(),
        wallix_group: Some("dev-admins".to_string()),
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
        ssh_agent_sock: String::new(),
    };

    let targets = build_expected_targets(&server);
    assert!(targets.contains(&"demo_user@default@app-db01.alpha.example.test:SSH".to_string()));
    assert!(targets.contains(&"demo_user@default@APP-DB01:SSH".to_string()));
    assert!(targets.contains(&"demo_user@default@PP-ALPHA-BD:SSH".to_string()));
}

#[test]
fn test_select_id_for_server_accepts_short_group_and_yaml_structure() {
    let entries = vec![WallixMenuEntry {
        id: "1".to_string(),
        target: "demo_user@default@PP-ALPHA-BD:SSH".to_string(),
        group: "APP-ALPHA_dev-admins".to_string(),
    }];

    let server = ResolvedServer {
        namespace: String::new(),
        group_name: "ALPHA".to_string(),
        env_name: "PP".to_string(),
        name: "bdd01".to_string(),
        host: "app-db01.alpha.example.test".to_string(),
        user: "demo_user".to_string(),
        port: 22,
        ssh_key: String::new(),
        ssh_options: vec![],
        default_mode: crate::config::ConnectionMode::Wallix,
        jump_host: None,
        bastion_host: Some("bastion.example.test".to_string()),
        bastion_user: Some("demo_user".to_string()),
        bastion_template: "{target_user}@%n:SSH:{bastion_user}".to_string(),
        wallix_group: Some("dev-admins".to_string()),
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
        ssh_agent_sock: String::new(),
    };

    assert_eq!(select_id_for_server(&entries, &server).unwrap(), "1");
}

#[test]
fn test_select_id_for_server_case_insensitive_target() {
    let entries = vec![WallixMenuEntry {
        id: "42".to_string(),
        target: "PCOLLIN@DEFAULT@PR-AUT-ANSCORE02:SSH".to_string(),
        group: "CES3S-ADMINS".to_string(),
    }];

    let server = ResolvedServer {
        namespace: String::new(),
        group_name: String::new(),
        env_name: String::new(),
        name: "anscore02".to_string(),
        host: "pr-aut-anscore02.ste.in.phm.education.gouv.fr".to_string(),
        user: "pcollin".to_string(),
        port: 22,
        ssh_key: String::new(),
        ssh_options: vec![],
        default_mode: crate::config::ConnectionMode::Wallix,
        jump_host: None,
        bastion_host: Some("ssh.in.phm.education.gouv.fr".to_string()),
        bastion_user: Some("pcollin".to_string()),
        bastion_template: "{target_user}@%n:SSH:{bastion_user}".to_string(),
        wallix_group: Some("ces3s-admins".to_string()),
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
        ssh_agent_sock: String::new(),
    };

    assert_eq!(select_id_for_server(&entries, &server).unwrap(), "42");
}

#[test]
fn test_select_id_for_server_accepts_prefixed_group_suffix() {
    let entries = vec![WallixMenuEntry {
        id: "640".to_string(),
        target: "demo_user@default@app-anscore02.example.test:SSH".to_string(),
        group: "APP-ANSCORE_dev-admins".to_string(),
    }];

    let server = ResolvedServer {
        namespace: String::new(),
        group_name: "Service Beta".to_string(),
        env_name: String::new(),
        name: "Service Beta".to_string(),
        host: "app-anscore02.example.test".to_string(),
        user: "demo_user".to_string(),
        port: 22,
        ssh_key: String::new(),
        ssh_options: vec![],
        default_mode: crate::config::ConnectionMode::Wallix,
        jump_host: None,
        bastion_host: Some("bastion.example.test".to_string()),
        bastion_user: Some("demo_user".to_string()),
        bastion_template: "{target_user}@%n:SSH:{bastion_user}".to_string(),
        wallix_group: Some("dev-admins".to_string()),
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
        ssh_agent_sock: String::new(),
    };

    assert_eq!(select_id_for_server(&entries, &server).unwrap(), "640");
}

#[test]
fn test_parse_realistic_wallix_output() {
    let output = r#"
╔════════════════════════════════════════════════════════════════════════════════╗
║                    Wallix Bastion - Interactive Menu                           ║
╚════════════════════════════════════════════════════════════════════════════════╝

   ID │ Cible                                │ Autorisation
───────┼──────────────────────────────────────┼────────────────────────
 0001  │ demo_user@default@APP-ALPHA-BD:SSH      │ APP-ALPHA_ops-admins
 0002  │ demo_user@default@APP-ALPHA-BD:SSH      │ APP-ALPHA_dev-admins
 0003  │ demo_user@default@OTHER-SERVER:SSH    │ APP-ALPHA_ops-admins
"#;
    let entries = parse_wallix_menu(output, &[]).expect("Should parse realistic output");
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].id, "0001");
    assert_eq!(entries[2].group, "APP-ALPHA_ops-admins");
}
