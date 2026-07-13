use super::*;
use crate::config::ConnectionMode;

fn base_server() -> ResolvedServer {
    ResolvedServer {
        namespace: String::new(),
        group_name: "G".into(),
        env_name: "E".into(),
        name: "srv".into(),
        host: "198.51.100.1".into(),
        user: "admin".into(),
        port: 22,
        ssh_key: String::new(),
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

// ── mode Direct ──────────────────────────────────────────────────────────

#[test]
fn direct_basic() {
    let s = base_server();
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();
    assert!(args.contains(&"-F".to_string()));
    assert!(args.contains(&"/dev/null".to_string()));
    assert!(args.contains(&"admin@198.51.100.1".to_string()));
    assert!(!args.contains(&"-v".to_string()));
}

#[test]
fn direct_verbose() {
    let s = base_server();
    let args = build_ssh_args(&s, ConnectionMode::Direct, true).unwrap();
    assert!(args.contains(&"-v".to_string()));
}

#[test]
fn direct_with_port_in_host() {
    let mut s = base_server();
    s.host = "198.51.100.1:2222".into();
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();
    assert!(args.contains(&"-p".to_string()));
    assert!(args.contains(&"2222".to_string()));
    assert!(args.contains(&"admin@198.51.100.1".to_string()));
}

#[test]
fn direct_with_port_field() {
    // Port via server.port (cas CLI --port ou ssh_port dans la config),
    // sans port embarqué dans la chaîne hôte.
    let mut s = base_server();
    s.port = 2222;
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();
    assert!(args.contains(&"-p".to_string()));
    assert!(args.contains(&"2222".to_string()));
    assert!(args.contains(&"admin@198.51.100.1".to_string()));
}

#[test]
fn direct_with_ssh_key() {
    let mut s = base_server();
    s.ssh_key = "~/.ssh/id_ed25519".into();
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();
    let key_pos = args.iter().position(|a| a == "-i").expect("-i present");
    assert!(!args[key_pos + 1].is_empty());
}

#[test]
fn direct_with_ssh_options() {
    let mut s = base_server();
    s.ssh_options = vec!["StrictHostKeyChecking=no".into(), "-T".into()];
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();
    // String option → prefixed with -o
    let o_pos = args.iter().position(|a| a == "-o").expect("-o present");
    assert_eq!(args[o_pos + 1], "StrictHostKeyChecking=no");
    // Flag option → passed as-is
    assert!(args.contains(&"-T".to_string()));
}

#[test]
fn direct_use_system_ssh_config() {
    let mut s = base_server();
    s.use_system_ssh_config = true;
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();
    assert!(!args.contains(&"-F".to_string()));
}

// ── mode Jump ────────────────────────────────────────────────────────────

#[test]
fn jump_basic() {
    let mut s = base_server();
    // jump_host contient déjà "user@host" (pré-formaté par resolve_server)
    s.jump_host = Some("juser@jump.example.com".into());
    let args = build_ssh_args(&s, ConnectionMode::Jump, false).unwrap();
    let j_pos = args.iter().position(|a| a == "-J").expect("-J present");
    assert_eq!(args[j_pos + 1], "juser@jump.example.com");
    assert!(args.contains(&"admin@198.51.100.1".to_string()));
}

#[test]
fn jump_with_port() {
    let mut s = base_server();
    s.jump_host = Some("juser@jump.example.com:2222".into());
    let args = build_ssh_args(&s, ConnectionMode::Jump, false).unwrap();
    let j_pos = args.iter().position(|a| a == "-J").expect("-J present");
    assert_eq!(args[j_pos + 1], "juser@jump.example.com:2222");
}

#[test]
fn jump_fallback_user() {
    // jump_user absent → l'utilisateur du serveur est déjà intégré au moment de la résolution
    let mut s = base_server();
    s.jump_host = Some("admin@jump.example.com".into()); // user=admin = server.user
    let args = build_ssh_args(&s, ConnectionMode::Jump, false).unwrap();
    let j_pos = args.iter().position(|a| a == "-J").expect("-J present");
    assert_eq!(args[j_pos + 1], "admin@jump.example.com");
}

#[test]
fn jump_multi_hop() {
    // Chaîne de deux sauts pré-formatée par resolve_server
    let mut s = base_server();
    s.jump_host = Some("juser@jump1.example.com,juser@jump2.example.com".into());
    let args = build_ssh_args(&s, ConnectionMode::Jump, false).unwrap();
    let j_pos = args.iter().position(|a| a == "-J").expect("-J present");
    assert_eq!(
        args[j_pos + 1],
        "juser@jump1.example.com,juser@jump2.example.com"
    );
    assert!(args.contains(&"admin@198.51.100.1".to_string()));
}

#[test]
fn jump_missing_host_returns_error() {
    let s = base_server(); // jump_host = None
    let err = build_ssh_args(&s, ConnectionMode::Jump, false).unwrap_err();
    assert!(err.to_string().contains("Jump host not configured"));
}

// ── mode Wallix ──────────────────────────────────────────────────────────

#[test]
fn wallix_basic() {
    let mut s = base_server();
    s.bastion_host = Some("bastion.example.com".into());
    s.bastion_user = Some("buser".into());
    let args = build_ssh_args(&s, ConnectionMode::Wallix, false).unwrap();
    let l_pos = args.iter().position(|a| a == "-l").expect("-l present");
    // template: {target_user}@%n:SSH:{bastion_user}
    assert_eq!(args[l_pos + 1], "admin@198.51.100.1:SSH:buser");
    assert!(args.contains(&"bastion.example.com".to_string()));
}

#[test]
fn wallix_with_port() {
    let mut s = base_server();
    s.bastion_host = Some("bastion.example.com:8022".into());
    s.bastion_user = Some("buser".into());
    let args = build_ssh_args(&s, ConnectionMode::Wallix, false).unwrap();
    assert!(args.contains(&"-p".to_string()));
    assert!(args.contains(&"8022".to_string()));
    assert!(args.contains(&"bastion.example.com".to_string()));
}

#[test]
fn wallix_fallback_user() {
    let mut s = base_server();
    s.bastion_host = Some("bastion.example.com".into());
    s.bastion_user = None; // fallback → "root"
    let args = build_ssh_args(&s, ConnectionMode::Wallix, false).unwrap();
    let l_pos = args.iter().position(|a| a == "-l").expect("-l present");
    assert!(args[l_pos + 1].ends_with(":SSH:root"));
}

#[test]
fn wallix_missing_host_returns_error() {
    let s = base_server(); // bastion_host = None
    let err = build_ssh_args(&s, ConnectionMode::Wallix, false).unwrap_err();
    assert!(err.to_string().contains("Wallix host not configured"));
}

#[test]
fn wallix_custom_template() {
    let mut s = base_server();
    s.bastion_host = Some("bastion.example.com".into());
    s.bastion_user = Some("buser".into());
    s.bastion_template = "{bastion_user}+{target_user}@{target_host}".into();
    let args = build_ssh_args(&s, ConnectionMode::Wallix, false).unwrap();
    let l_pos = args.iter().position(|a| a == "-l").expect("-l present");
    assert_eq!(args[l_pos + 1], "buser+admin@198.51.100.1");
}

#[test]
fn wallix_bastion_args_use_bastion_identity_only_for_menu_automation() {
    let mut s = base_server();
    s.bastion_host = Some("bastion.example.com:8022".into());
    s.bastion_user = Some("demo_user".into());
    let args = build_wallix_bastion_args(&s, false).unwrap();

    assert!(args.contains(&"-l".to_string()));
    // Login includes target host for server-side Wallix filtering (avoids paginating all entries).
    assert!(
        args.iter()
            .any(|a| a.starts_with("demo_user@") && a.contains(":SSH:demo_user"))
    );
    assert!(args.contains(&"-p".to_string()));
    assert!(args.contains(&"8022".to_string()));
    assert_eq!(args.last().unwrap(), "bastion.example.com");
}

#[test]
fn wallix_menu_prompt_detection_supports_ascii_prompt() {
    assert!(contains_wallix_prompt(
        "Tapez h pour l'aide, ctrl-D pour quitter\n > "
    ));
}

#[test]
fn wallix_target_address_prompt_detection_supports_french_prompt() {
    assert!(contains_wallix_target_address_prompt(
        "Account successfully checked out\nAdresse cible (dans 10.242.23.24/29): "
    ));
}

#[test]
fn wallix_return_selector_prompt_detection_supports_french_prompt() {
    assert!(contains_wallix_return_selector_prompt(
        "Session fermée, retour au sélecteur ? [o/N]"
    ));
}

#[test]
fn wallix_page_position_parser_reads_page_numbers() {
    let line = "| ID | Cible (page 1/16)                       | Autorisation";
    assert_eq!(parse_wallix_page_position(line), Some((1, 16)));
}

// ── invariant destination ─────────────────────────────────────────────────

/// Garantit que la destination (`user@host`) est toujours le dernier argument,
/// quelle que soit la combinaison d'options. Cet invariant est utilisé par
/// `build_tunnel_args` et `probe` pour insérer des options juste avant la cible.
#[test]
fn destination_is_last() {
    // Direct avec clé + options + port non-standard
    let mut s = base_server();
    s.ssh_key = "~/.ssh/id_ed25519".into();
    s.ssh_options = vec!["StrictHostKeyChecking=no".into(), "-T".into()];
    s.port = 2222;
    let args = build_ssh_args(&s, ConnectionMode::Direct, true).unwrap();
    assert_eq!(args.last().unwrap(), "admin@198.51.100.1");

    // Jump avec clé + port dans l'hôte
    let mut s2 = base_server();
    s2.ssh_key = "~/.ssh/id_ed25519".into();
    s2.host = "198.51.100.1:2222".into();
    s2.jump_host = Some("juser@jump.example.com:22".into());
    let args2 = build_ssh_args(&s2, ConnectionMode::Jump, false).unwrap();
    assert_eq!(args2.last().unwrap(), "admin@198.51.100.1");

    // Direct minimal — destination = dernier arg même sans options
    let s3 = base_server();
    let args3 = build_ssh_args(&s3, ConnectionMode::Direct, false).unwrap();
    assert_eq!(args3.last().unwrap(), "admin@198.51.100.1");
}

// ── StrictHostKeyChecking=accept-new ──────────────────────────────────────

#[test]
fn accept_new_injected_when_no_strict_host_option() {
    let s = base_server(); // use_system_ssh_config=false, ssh_options=[]
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();
    // Cherche accept-new parmi toutes les valeurs -o
    let has_accept_new = args
        .windows(2)
        .any(|w| w[0] == "-o" && w[1] == "StrictHostKeyChecking=accept-new");
    assert!(has_accept_new, "accept-new doit être injecté: {args:?}");
    // La destination reste dernière malgré l'injection
    assert_eq!(args.last().unwrap(), "admin@198.51.100.1");
}

#[test]
fn accept_new_not_injected_when_user_sets_strict_host_no() {
    let mut s = base_server();
    s.ssh_options = vec!["StrictHostKeyChecking=no".into()];
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();
    let count = args
        .windows(2)
        .filter(|w| w[0] == "-o" && w[1].to_ascii_lowercase().contains("stricthostkeychecking"))
        .count();
    assert_eq!(
        count, 1,
        "une seule option StrictHostKeyChecking attendue: {args:?}"
    );
}

#[test]
fn accept_new_not_injected_when_use_system_ssh_config() {
    let mut s = base_server();
    s.use_system_ssh_config = true;
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();
    let has_accept_new = args
        .windows(2)
        .any(|w| w[0] == "-o" && w[1] == "StrictHostKeyChecking=accept-new");
    assert!(
        !has_accept_new,
        "ne doit pas injecter avec use_system_ssh_config: {args:?}"
    );
}

#[test]
fn accept_new_injected_for_wallix_bastion_args() {
    let mut s = base_server();
    s.bastion_host = Some("bastion.example.com".into());
    s.bastion_user = Some("buser".into());
    let args = build_wallix_bastion_args(&s, false).unwrap();
    let has_accept_new = args
        .windows(2)
        .any(|w| w[0] == "-o" && w[1] == "StrictHostKeyChecking=accept-new");
    assert!(
        has_accept_new,
        "accept-new doit être injecté pour Wallix: {args:?}"
    );
}

// ── ControlMaster ─────────────────────────────────────────────────────────

#[test]
fn control_master_inactive_when_disabled() {
    // control_master: false → retourne false sans vérification filesystem
    let s = base_server();
    assert!(!is_control_master_active(&s));
}

#[test]
fn control_master_inactive_when_socket_absent() {
    let mut s = base_server();
    s.control_master = true;
    s.control_path = "/tmp/susshi-test-nonexistent-socket-%h_%p_%r".into();
    assert!(!is_control_master_active(&s));
}

#[test]
fn control_master_inactive_when_path_empty() {
    let mut s = base_server();
    s.control_master = true;
    s.control_path = String::new();
    assert!(!is_control_master_active(&s));
}

// ── askpass security tests ────────────────────────────────────────────────

/// The escaping logic for single quotes must produce the correct sh sequence '\''
/// so that credentials with single quotes don't break the printf argument.
#[test]
fn askpass_escape_logic_replaces_single_quotes() {
    let cred = "it's a 'secret'";
    let escaped = cred.replace('\'', r"'\''");
    // Each original ' must become '\'' (end quote, escaped quote, reopen quote)
    assert_eq!(escaped, r"it'\''s a '\''secret'\''");
    // The original unescaped character must not appear in the middle of the string
    // (it should only appear as part of '\'' sequences)
    assert!(
        !escaped.contains("it's"),
        "original unescaped form must not survive"
    );
}

/// Credentials without single quotes are not modified by the escaping logic.
#[test]
fn askpass_escape_logic_no_single_quotes_unchanged() {
    let cred = "plainpassword123!@#";
    let escaped = cred.replace('\'', r"'\''");
    assert_eq!(escaped, cred);
}

/// The askpass script file must be created with 0o700 permissions (owner-only).
#[test]
#[cfg(unix)]
fn askpass_file_has_700_permissions() {
    use std::os::unix::fs::PermissionsExt as _;
    let path = setup_askpass_script("hunter2_unique_permissions_test").unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        mode & 0o777,
        0o700,
        "askpass script must be owner-executable only (0o700)"
    );
}
