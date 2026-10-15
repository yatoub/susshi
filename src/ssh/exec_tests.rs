use super::*;
use std::time::Duration;

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
        wallix_authorizations: vec![],
        wallix_header_columns: vec![],
    }
}

fn wallix_server() -> ResolvedServer {
    let mut s = base_server();
    s.host = "srv01.example.fr".into();
    s.user = "jdoe".into();
    s.bastion_host = Some("bastion.example.fr".into());
    s.bastion_user = Some("jdoe".into());
    s
}

// ── Direct / Jump : commande en argument ─────────────────────────────────────

#[test]
fn exec_args_direct_command_after_destination() {
    let args = build_exec_args(&base_server(), ConnectionMode::Direct, "uptime").unwrap();
    let n = args.len();
    assert_eq!(args[n - 2], "admin@198.51.100.1");
    assert_eq!(args[n - 1], "uptime");
    for opt in ["-n", "BatchMode=yes", "ConnectTimeout=10"] {
        assert!(args.contains(&opt.to_string()), "{opt} manquant : {args:?}");
    }
}

/// Le mode Jump est respecté (la commande ad-hoc l'ignorait).
#[test]
fn exec_args_jump_keeps_jump_host() {
    let mut s = base_server();
    s.jump_host = Some("ops@jump.example.fr".into());
    let args = build_exec_args(&s, ConnectionMode::Jump, "uptime").unwrap();
    let j = args.iter().position(|a| a == "-J").expect("-J");
    assert_eq!(args[j + 1], "ops@jump.example.fr");
    assert_eq!(args.last().unwrap(), "uptime");
}

/// Les options SSH du serveur sont transmises (la commande ad-hoc les ignorait).
#[test]
fn exec_args_keep_server_ssh_options() {
    let mut s = base_server();
    s.ssh_options = vec!["ServerAliveInterval=30".into()];
    let args = build_exec_args(&s, ConnectionMode::Direct, "uptime").unwrap();
    assert!(
        args.contains(&"ServerAliveInterval=30".to_string()),
        "{args:?}"
    );
}

// ── Wallix : session shell -T, commande sur stdin ───────────────────────────

#[test]
fn wallix_exec_args_shell_session_without_command() {
    let args = build_wallix_exec_args(&wallix_server(), Some("PR-APP_admins")).unwrap();
    assert!(args.contains(&"-T".to_string()), "{args:?}");
    assert!(!args.contains(&"-n".to_string()), "stdin requis : {args:?}");
    assert!(args.contains(&"BatchMode=yes".to_string()), "{args:?}");
    assert!(
        args.contains(&"jdoe@srv01.example.fr+SSH+PR-APP_admins+jdoe".to_string()),
        "{args:?}"
    );
    assert_eq!(args.last().unwrap(), "bastion.example.fr");
}

#[test]
fn run_remote_wallix_without_bastion_fails() {
    let mut s = wallix_server();
    s.bastion_host = None;
    let err = run_remote(&s, ConnectionMode::Wallix, "uptime", T).unwrap_err();
    assert!(err.to_string().contains("Wallix host"), "{err}");
}

// ── Fin de commande via marqueur (le bastion ne ferme pas la session) ──────

const T: Duration = Duration::from_secs(5);

#[test]
fn shell_input_appends_end_marker_and_exit() {
    let input = shell_input("uptime");
    assert!(input.starts_with("uptime\n"), "{input}");
    assert!(
        input.contains(&format!("echo \"{END_MARKER}$?\"")),
        "{input}"
    );
    assert!(input.ends_with("exit\n"), "{input}");
}

#[test]
fn split_end_marker_extracts_output_and_code() {
    let raw = format!("a\nb\n{END_MARKER}0\n");
    assert_eq!(split_end_marker(&raw), Some(("a\nb\n".to_string(), 0)));
    let raw = format!("x\n{END_MARKER}127\n");
    assert_eq!(split_end_marker(&raw), Some(("x\n".to_string(), 127)));
}

/// Sortie sans saut de ligne final : le marqueur est collé à la dernière ligne.
#[test]
fn split_end_marker_glued_to_last_line() {
    let raw = format!("no-newline{END_MARKER}2\n");
    assert_eq!(split_end_marker(&raw), Some(("no-newline".to_string(), 2)));
}

#[test]
fn split_end_marker_incomplete() {
    assert_eq!(split_end_marker("partial output"), None);
    // Code pas encore reçu en entier (pas de fin de ligne).
    assert_eq!(split_end_marker(&format!("x\n{END_MARKER}1")), None);
}

/// La ligne envoyée sur stdin est exécutée par le shell ; le marqueur est
/// retiré de la sortie et porte le code de sortie.
#[test]
#[cfg(unix)]
fn run_process_feeds_command_to_shell() {
    let out = run_process("sh", &[], Some("echo hello; echo oops >&2"), T).unwrap();
    assert_eq!(out.stdout, "hello\n");
    assert_eq!(out.stderr, "oops\n");
    assert!(out.success);
}

#[test]
#[cfg(unix)]
fn run_process_exit_code_from_marker() {
    let out = run_process("sh", &[], Some("false"), T).unwrap();
    assert!(!out.success);
}

/// Cas Wallix : le shell distant a fini mais la session reste ouverte
/// (EOF non transmis / canal non fermé). Le marqueur suffit à rendre la main.
#[test]
#[cfg(unix)]
fn run_process_returns_on_marker_even_if_session_stays_open() {
    let start = std::time::Instant::now();
    let args = ["-c".to_string(), "cat | sh; sleep 30; true".to_string()];
    let out = run_process("sh", &args, Some("echo done"), T).unwrap();
    assert_eq!(out.stdout, "done\n");
    assert!(out.success);
    assert!(
        start.elapsed() < Duration::from_secs(3),
        "{:?}",
        start.elapsed()
    );
}

/// Un processus qui ne rend jamais la main est tué à l'expiration du délai.
#[test]
#[cfg(unix)]
fn run_process_times_out() {
    let start = std::time::Instant::now();
    let args = ["-c".to_string(), "echo partial; sleep 30; true".to_string()];
    let err = run_process("sh", &args, None, Duration::from_millis(300)).unwrap_err();
    assert!(err.to_string().contains("délai"), "{err}");
    assert!(err.to_string().contains("partial"), "{err}");
    assert!(
        start.elapsed() < Duration::from_secs(3),
        "{:?}",
        start.elapsed()
    );
}

/// Le processus se termine mais un descendant garde stdout ouvert (ex. maître
/// `ControlPersist` d'OpenSSH) : la sortie déjà reçue est rendue sans attendre.
#[test]
#[cfg(unix)]
fn run_process_does_not_wait_for_descendants_holding_pipes() {
    let start = std::time::Instant::now();
    let args = ["-c".to_string(), "echo done; sleep 30 &".to_string()];
    let out = run_process("sh", &args, None, T).unwrap();
    assert_eq!(out.stdout, "done\n");
    assert!(
        start.elapsed() < Duration::from_secs(3),
        "{:?}",
        start.elapsed()
    );
}

/// Sans entrée (Direct/Jump) : stdin vide, fin à la sortie du processus.
#[test]
#[cfg(unix)]
fn run_process_without_input_uses_empty_stdin() {
    let args = ["-c".to_string(), "cat; echo done; exit 4".to_string()];
    let out = run_process("sh", &args, None, T).unwrap();
    assert_eq!(out.stdout, "done\n");
    assert!(!out.success);
}

#[test]
fn run_process_missing_program_fails() {
    assert!(run_process("susshi-no-such-binary", &[], None, T).is_err());
}
