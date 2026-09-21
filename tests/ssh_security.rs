//! Security regression tests for `ssh::client::build_ssh_args`.
//!
//! Verifies that dangerous characters in server fields (hostname, user,
//! key paths, SSH options, Wallix strings) are never interpreted by a shell —
//! they are passed as literal elements of the `Vec<String>` argument list.

use susshi::config::{ConnectionMode, ResolvedServer};
use susshi::ssh::client::build_ssh_args;

fn base_server() -> ResolvedServer {
    ResolvedServer {
        namespace: String::new(),
        group_name: "sec".into(),
        env_name: "test".into(),
        name: "srv".into(),
        host: "10.0.0.1".into(),
        user: "ops".into(),
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
        wallix_header_columns: vec![],
    }
}

// ─── Hostnames with shell metacharacters ─────────────────────────────────────

/// A hostname containing shell metacharacters must be a single literal Vec element.
#[test]
fn hostname_with_semicolon_is_single_arg() {
    let mut s = base_server();
    s.host = "10.0.0.1; rm -rf /".into();
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();
    let last = args.last().unwrap();
    assert_eq!(last, "ops@10.0.0.1; rm -rf /");
    // The semicolon must not have caused a new argument to appear
    let semicolons_in_args = args.iter().filter(|a| a.contains("; rm")).count();
    assert_eq!(
        semicolons_in_args, 1,
        "semicolon should appear in exactly one arg"
    );
}

/// Backtick command substitution in hostname stays literal.
#[test]
fn hostname_with_backticks_is_literal() {
    let mut s = base_server();
    s.host = "10.0.0.1`whoami`".into();
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();
    assert_eq!(args.last().unwrap(), "ops@10.0.0.1`whoami`");
}

/// Dollar sign / subshell syntax in hostname stays literal.
#[test]
fn hostname_with_dollar_subshell_is_literal() {
    let mut s = base_server();
    s.host = "10.0.0.1$(id)".into();
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();
    assert_eq!(args.last().unwrap(), "ops@10.0.0.1$(id)");
}

// ─── Usernames with shell metacharacters ─────────────────────────────────────

/// A username containing a semicolon stays as one element.
#[test]
fn username_with_semicolon_is_single_arg() {
    let mut s = base_server();
    s.user = "admin; evil".into();
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();
    assert_eq!(args.last().unwrap(), "admin; evil@10.0.0.1");
}

/// A username with an embedded newline does not split into multiple args.
#[test]
fn username_with_newline_is_single_arg() {
    let mut s = base_server();
    s.user = "admin\nmalicious".into();
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();
    assert_eq!(args.last().unwrap(), "admin\nmalicious@10.0.0.1");
}

// ─── SSH key paths ────────────────────────────────────────────────────────────

/// Tilde in the key path is expanded before being passed to ssh.
#[test]
fn key_path_tilde_is_expanded() {
    let mut s = base_server();
    s.ssh_key = "~/.ssh/id_ed25519".into();
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();
    let i_pos = args.iter().position(|a| a == "-i").expect("-i expected");
    assert!(
        !args[i_pos + 1].starts_with('~'),
        "tilde must be expanded, got: {}",
        args[i_pos + 1]
    );
    assert!(args[i_pos + 1].ends_with("/.ssh/id_ed25519"));
}

/// A relative key path with `../` traversal is passed through unchanged — susshi
/// does not restrict key paths; the ssh binary is responsible for rejecting invalid paths.
#[test]
fn key_path_dotdot_passed_through_to_ssh() {
    let mut s = base_server();
    s.ssh_key = "../../etc/passwd".into();
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();
    let i_pos = args.iter().position(|a| a == "-i").expect("-i expected");
    assert_eq!(args[i_pos + 1], "../../etc/passwd");
}

// ─── SSH options passthrough ──────────────────────────────────────────────────

/// An SSH option containing a semicolon is passed as two args (-o and the value),
/// not split into multiple arguments at the shell level.
#[test]
fn ssh_option_with_semicolon_is_two_args() {
    let mut s = base_server();
    s.ssh_options = vec!["ServerAliveInterval=30; rm -rf /".into()];
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();
    let o_pos = args.iter().position(|a| a == "-o").expect("-o expected");
    // The value must be the single next argument
    assert_eq!(args[o_pos + 1], "ServerAliveInterval=30; rm -rf /");
    // No additional splitting: the '; rm' must not appear as a standalone arg
    assert!(
        !args.iter().any(|a| a.trim_start().starts_with("rm")),
        "rm must not appear as a separate arg"
    );
}

/// When the user provides `StrictHostKeyChecking`, susshi does not inject its own default.
#[test]
fn user_strict_host_checking_prevents_auto_inject() {
    let mut s = base_server();
    s.ssh_options = vec!["StrictHostKeyChecking=no".into()];
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();
    let accept_new_count = args
        .iter()
        .filter(|a| a.contains("StrictHostKeyChecking=accept-new"))
        .count();
    assert_eq!(
        accept_new_count, 0,
        "accept-new must not be injected when user already sets StrictHostKeyChecking"
    );
}

// ─── Wallix login string ──────────────────────────────────────────────────────

/// A wallix_group containing a semicolon is embedded literally in the `-l` value,
/// not interpreted as a shell command separator.
#[test]
fn wallix_group_with_semicolon_is_literal_in_login_string() {
    let mut s = base_server();
    s.bastion_host = Some("bastion.example.com".into());
    s.bastion_user = Some("bops".into());
    s.wallix_group = Some("group; malicious".into());
    let args = build_ssh_args(&s, ConnectionMode::Wallix, false).unwrap();
    let l_pos = args.iter().position(|a| a == "-l").expect("-l expected");
    let login = &args[l_pos + 1];
    // The semicolon must be embedded inside the single -l value
    assert!(login.contains("group; malicious"), "login string: {login}");
    // Verify the total number of arguments didn't grow from the injection
    assert!(
        !args.iter().any(|a| a.trim_start().starts_with("malicious")),
        "malicious must not appear as a standalone arg"
    );
}

/// A wallix_group containing a newline stays as part of the single `-l` value.
#[test]
fn wallix_group_with_newline_is_single_arg() {
    let mut s = base_server();
    s.bastion_host = Some("bastion.example.com".into());
    s.bastion_user = Some("bops".into());
    s.wallix_group = Some("group\nevil-option".into());
    let args = build_ssh_args(&s, ConnectionMode::Wallix, false).unwrap();
    let l_pos = args.iter().position(|a| a == "-l").expect("-l expected");
    let login = &args[l_pos + 1];
    assert!(
        login.contains('\n'),
        "newline must be inside the -l value, got: {login:?}"
    );
}
