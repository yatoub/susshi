//! Tests d'intégration pour `ssh::tunnel` — construction des arguments et
//! cycle de vie d'un `TunnelHandle`.
//!
//! Aucun serveur SSH réel n'est requis. Les tests `TunnelHandle::new` +
//! `is_running` + `poll` sont purement en mémoire. Les tests sur
//! `build_tunnel_args` vérifient la structure des arguments sans lancer `ssh`.

use susshi::config::{ConnectionMode, ResolvedServer, TunnelConfig};
use susshi::ssh::tunnel::{TunnelHandle, TunnelStatus, build_tunnel_args};

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn base_server() -> ResolvedServer {
    ResolvedServer {
        namespace: String::new(),
        group_name: "integration".into(),
        env_name: "test".into(),
        name: "srv".into(),
        host: "198.51.100.10".into(),
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

fn pg_tunnel() -> TunnelConfig {
    TunnelConfig {
        local_port: 5433,
        remote_host: "127.0.0.1".into(),
        remote_port: 5432,
        label: "PostgreSQL".into(),
    }
}

// ─── TunnelHandle — cycle de vie ─────────────────────────────────────────────

/// Un handle créé via `TunnelHandle::new` est dans l'état `Idle` par défaut.
#[test]
fn new_is_idle() {
    let h = TunnelHandle::new(pg_tunnel(), Some(0), 0);
    assert!(
        matches!(h.status, TunnelStatus::Idle),
        "statut attendu : Idle, obtenu : {:?}",
        h.status
    );
}

/// `is_running()` retourne `false` pour un handle `Idle`.
#[test]
fn is_running_false_when_idle() {
    let h = TunnelHandle::new(pg_tunnel(), None, 0);
    assert!(!h.is_running(), "is_running doit retourner false pour Idle");
}

/// `poll()` sur un handle `Idle` (sans processus enfant) retourne `false`
/// sans paniquer ni modifier le statut.
#[test]
fn poll_returns_false_without_child() {
    let mut h = TunnelHandle::new(pg_tunnel(), None, 0);
    let became_dead = h.poll();
    assert!(!became_dead, "poll sans child doit retourner false");
    assert!(
        matches!(h.status, TunnelStatus::Idle),
        "le statut ne doit pas changer après poll sur Idle"
    );
}

/// Les champs `config`, `yaml_index` et `user_idx` sont correctement stockés.
#[test]
fn new_stores_config_and_indices() {
    let t = pg_tunnel();
    let h = TunnelHandle::new(t.clone(), Some(3), 7);
    assert_eq!(h.config.local_port, t.local_port);
    assert_eq!(h.config.remote_host, t.remote_host);
    assert_eq!(h.config.remote_port, t.remote_port);
    assert_eq!(h.yaml_index, Some(3));
    assert_eq!(h.user_idx, 7);
}

/// Un handle sans origine YAML (`yaml_index = None`) est valide.
#[test]
fn new_without_yaml_index() {
    let h = TunnelHandle::new(pg_tunnel(), None, 0);
    assert!(h.yaml_index.is_none(), "yaml_index doit être None");
}

// ─── build_tunnel_args — structure des arguments ─────────────────────────────

/// Les arguments d'un tunnel direct contiennent `-N`, `-L port:host:port`
/// et la destination en dernière position.
#[test]
fn build_tunnel_args_structure() {
    let s = base_server();
    let t = pg_tunnel();
    let args = build_tunnel_args(&s, ConnectionMode::Direct, &t).unwrap();

    // -N : pas de commande distante
    assert!(args.contains(&"-N".to_string()), "-N attendu");

    // -L localPort:remoteHost:remotePort
    let l_pos = args.iter().position(|a| a == "-L").expect("-L attendu");
    assert_eq!(
        args[l_pos + 1],
        "5433:127.0.0.1:5432",
        "format -L incorrect"
    );

    // ExitOnForwardFailure doit être activé
    assert!(
        args.iter()
            .any(|a: &String| a.contains("ExitOnForwardFailure")),
        "ExitOnForwardFailure attendu"
    );

    // La destination reste toujours en dernière position (invariant critique)
    assert_eq!(
        args.last().unwrap(),
        "ops@198.51.100.10",
        "destination doit être en dernière position"
    );
}

/// Mode Jump : `-J` est présent et la destination reste en dernière position.
#[test]
fn build_tunnel_args_jump_keeps_j_and_destination_last() {
    let mut s = base_server();
    s.jump_host = Some("jops@jump.example.com".into());
    let t = pg_tunnel();
    let args = build_tunnel_args(&s, ConnectionMode::Jump, &t).unwrap();

    assert!(args.contains(&"-J".to_string()), "-J attendu en mode Jump");
    assert!(args.contains(&"-N".to_string()), "-N attendu");
    assert_eq!(
        args.last().unwrap(),
        "ops@198.51.100.10",
        "destination doit être en dernière position"
    );
}

/// Mode Wallix est refusé (tunnels non disponibles via Wallix).
#[test]
fn build_tunnel_args_wallix_rejected() {
    let s = base_server();
    let err = build_tunnel_args(&s, ConnectionMode::Wallix, &pg_tunnel()).unwrap_err();
    assert!(
        err.to_string().contains("Wallix"),
        "erreur Wallix attendue, obtenu : {}",
        err
    );
}

/// Les ssh_options du serveur (options et flags) précèdent toujours la destination.
#[test]
fn build_tunnel_args_options_before_destination() {
    let mut s = base_server();
    s.ssh_key = "~/.ssh/id_ed25519".into();
    s.ssh_options = vec!["ServerAliveInterval=30".into()];
    let t = pg_tunnel();
    let args = build_tunnel_args(&s, ConnectionMode::Direct, &t).unwrap();

    let dest_pos = args
        .iter()
        .rposition(|a| a == "ops@198.51.100.10")
        .expect("destination absente");
    let opt_pos = args
        .iter()
        .position(|a| a == "ServerAliveInterval=30")
        .expect("option absente");

    assert!(
        opt_pos < dest_pos,
        "les options SSH doivent précéder la destination"
    );
}

/// Le format du flag `-L` est `localPort:remoteHost:remotePort`.
#[test]
fn build_tunnel_args_l_flag_format_with_named_host() {
    let s = base_server();
    let t = TunnelConfig {
        local_port: 15432,
        remote_host: "db.internal.example.com".into(),
        remote_port: 5432,
        label: "DB interne".into(),
    };
    let args = build_tunnel_args(&s, ConnectionMode::Direct, &t).unwrap();
    let l_pos = args.iter().position(|a| a == "-L").expect("-L attendu");
    assert_eq!(
        args[l_pos + 1],
        "15432:db.internal.example.com:5432",
        "format -L incorrect avec nom d'hôte"
    );
}
