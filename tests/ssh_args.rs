//! Tests d'intégration pour `ssh::client::build_ssh_args`.
//!
//! Ces tests exercent la fonction `build_ssh_args` depuis l'API publique afin
//! de garantir que les scénarios courants produisent les arguments SSH attendus.
//! Aucun serveur SSH réel n'est requis — tout est purement en mémoire.

use susshi::config::{ConnectionMode, ResolvedServer};
use susshi::ssh::client::build_ssh_args;

// ─── Helper ──────────────────────────────────────────────────────────────────

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

// ─── Mode Direct ─────────────────────────────────────────────────────────────

/// Un serveur minimal produit `-F /dev/null` + `user@host` sans options superflues.
#[test]
fn direct_minimal() {
    let s = base_server();
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();

    assert!(args.contains(&"-F".to_string()), "-F attendu");
    assert!(args.contains(&"/dev/null".to_string()), "/dev/null attendu");
    assert!(
        args.contains(&"ops@198.51.100.10".to_string()),
        "destination attendue"
    );
    // Pas d'options superflues pour un serveur port-22 sans clé
    assert!(
        !args.contains(&"-p".to_string()),
        "-p inattendu pour port 22"
    );
    assert!(!args.contains(&"-i".to_string()), "-i inattendu sans clé");
    assert!(!args.contains(&"-v".to_string()), "-v inattendu");
}

/// La clé SSH est passée avec `-i` et le tilde est expandé.
#[test]
fn direct_with_key() {
    let mut s = base_server();
    s.ssh_key = "~/.ssh/id_ed25519".into();
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();

    let i_pos = args.iter().position(|a| a == "-i").expect("-i attendu");
    // Le tilde doit avoir été expandé (shellexpand::tilde)
    assert!(
        !args[i_pos + 1].starts_with('~'),
        "le tilde doit être expandé"
    );
    assert!(
        args[i_pos + 1].ends_with("/.ssh/id_ed25519"),
        "chemin de clé incorrect"
    );
}

/// Un port non-standard est ajouté avec `-p`.
#[test]
fn direct_with_port() {
    let mut s = base_server();
    s.port = 2222;
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();

    let p_pos = args.iter().position(|a| a == "-p").expect("-p attendu");
    assert_eq!(args[p_pos + 1], "2222", "valeur du port incorrecte");
    // La destination ne doit pas contenir le port
    assert_eq!(
        args.last().unwrap(),
        "ops@198.51.100.10",
        "destination incorrecte"
    );
}

/// Un port embarqué dans la chaîne `host:port` est extrait et transmis via `-p`.
#[test]
fn direct_with_port_in_host_string() {
    let mut s = base_server();
    s.host = "198.51.100.10:2222".into();
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();

    assert!(args.contains(&"-p".to_string()), "-p attendu");
    assert!(args.contains(&"2222".to_string()), "valeur 2222 attendue");
    // La destination doit utiliser le host sans le port
    assert_eq!(args.last().unwrap(), "ops@198.51.100.10");
}

/// Les `ssh_options` scalaires sont préfixées par `-o`, les flags (commençant par `-`) passent tels quels.
#[test]
fn direct_with_options() {
    let mut s = base_server();
    s.ssh_options = vec!["ServerAliveInterval=30".into(), "-T".into()];
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();

    let o_pos = args.iter().position(|a| a == "-o").expect("-o attendu");
    assert_eq!(
        args[o_pos + 1],
        "ServerAliveInterval=30",
        "option scalaire incorrecte"
    );
    assert!(
        args.contains(&"-T".to_string()),
        "flag -T doit passer tel quel"
    );
}

/// `use_system_ssh_config: true` supprime le `-F /dev/null`.
#[test]
fn system_ssh_config() {
    let mut s = base_server();
    s.use_system_ssh_config = true;
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();

    assert!(
        !args.contains(&"-F".to_string()),
        "-F ne doit pas être présent quand use_system_ssh_config=true"
    );
}

// ─── Mode Jump ───────────────────────────────────────────────────────────────

/// Mode Jump correct : `-J user@host` suivi de la destination cible.
#[test]
fn jump_host() {
    let mut s = base_server();
    s.jump_host = Some("jops@jump.infra.example.com".into());
    let args = build_ssh_args(&s, ConnectionMode::Jump, false).unwrap();

    let j_pos = args.iter().position(|a| a == "-J").expect("-J attendu");
    assert_eq!(args[j_pos + 1], "jops@jump.infra.example.com");
    assert_eq!(args.last().unwrap(), "ops@198.51.100.10");
}

/// Mode Jump sans `jump_host` configuré retourne une erreur explicite.
#[test]
fn jump_no_host() {
    let s = base_server(); // jump_host = None
    let err = build_ssh_args(&s, ConnectionMode::Jump, false).unwrap_err();
    assert!(
        err.to_string().contains("Jump host not configured"),
        "message d'erreur attendu, obtenu : {}",
        err
    );
}

// ─── Mode Wallix ─────────────────────────────────────────────────────────────

/// Mode Wallix : le template est correctement substitué dans `-l`.
#[test]
fn wallix_template() {
    let mut s = base_server();
    s.bastion_host = Some("bastion.corp.example.com".into());
    s.bastion_user = Some("bops".into());
    s.wallix_group = Some("PR-OND-BD_crtech-admins".into());
    // template par défaut : {target_user}@%n:SSH:{bastion_user}
    let args = build_ssh_args(&s, ConnectionMode::Wallix, false).unwrap();

    let l_pos = args.iter().position(|a| a == "-l").expect("-l attendu");
    assert_eq!(
        args[l_pos + 1],
        "ops@198.51.100.10:SSH:PR-OND-BD_crtech-admins:bops",
        "template Wallix incorrect"
    );
    assert!(
        args.contains(&"bastion.corp.example.com".to_string()),
        "bastion host absent"
    );
}

/// Mode Wallix : si aucun groupe n'est résolu, la chaîne reste valide sans segment d'autorisation.
#[test]
fn wallix_template_without_group() {
    let mut s = base_server();
    s.bastion_host = Some("bastion.corp.example.com".into());
    s.bastion_user = Some("bops".into());
    s.wallix_group = None;

    let args = build_ssh_args(&s, ConnectionMode::Wallix, false).unwrap();
    let l_pos = args.iter().position(|a| a == "-l").expect("-l attendu");

    assert_eq!(
        args[l_pos + 1],
        "ops@198.51.100.10:SSH:bops",
        "chaîne Wallix inattendue sans groupe"
    );
}

/// Mode Wallix sans `bastion_host` retourne une erreur explicite.
#[test]
fn wallix_no_host() {
    let s = base_server(); // bastion_host = None
    let err = build_ssh_args(&s, ConnectionMode::Wallix, false).unwrap_err();
    assert!(
        err.to_string().contains("Wallix host not configured"),
        "message d'erreur attendu, obtenu : {}",
        err
    );
}

// ─── Invariant destination ────────────────────────────────────────────────────

/// La destination (`user@host`) est **toujours** le dernier argument de la liste,
/// quels que soient les modes, clés, options et ports configurés.
///
/// Cet invariant est critique : `build_tunnel_args` et `probe` s'en servent pour
/// insérer leurs propres options juste avant la cible en faisant un `args.pop()`.
#[test]
fn destination_is_last() {
    // Direct + clé + options + port non-standard + verbose
    let mut s = base_server();
    s.ssh_key = "~/.ssh/id_ed25519".into();
    s.ssh_options = vec![
        "StrictHostKeyChecking=no".into(),
        "-T".into(),
        "BatchMode=yes".into(),
    ];
    s.port = 22222;
    let args = build_ssh_args(&s, ConnectionMode::Direct, true).unwrap();
    assert_eq!(
        args.last().unwrap(),
        "ops@198.51.100.10",
        "Direct : destination doit être en dernière position"
    );

    // Jump + clé + port dans l'hôte
    let mut s2 = base_server();
    s2.ssh_key = "~/.ssh/prod_ed25519".into();
    s2.host = "198.51.100.10:22".into();
    s2.jump_host = Some("jops@jump.example.com:2222".into());
    let args2 = build_ssh_args(&s2, ConnectionMode::Jump, false).unwrap();
    assert_eq!(
        args2.last().unwrap(),
        "ops@198.51.100.10",
        "Jump : destination doit être en dernière position"
    );
}

#[test]
fn agent_forwarding_adds_flag() {
    let mut s = base_server();
    s.agent_forwarding = true;
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();
    assert!(
        args.contains(&"-A".to_string()),
        "agent_forwarding doit ajouter -A"
    );
}

#[test]
fn no_agent_forwarding_by_default() {
    let s = base_server();
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();
    assert!(
        !args.contains(&"-A".to_string()),
        "-A ne doit pas être présent par défaut"
    );
}

/// Un certificat SSH est passé avec un second `-i` après la clé.
#[test]
fn ssh_cert_adds_second_identity_flag() {
    let mut s = base_server();
    s.ssh_key = "~/.ssh/id_ed25519".into();
    s.ssh_cert = "~/.ssh/id_ed25519-cert.pub".into();
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();

    let i_positions: Vec<usize> = args
        .iter()
        .enumerate()
        .filter(|(_, a)| *a == "-i")
        .map(|(i, _)| i)
        .collect();

    assert_eq!(i_positions.len(), 2, "deux `-i` attendus (clé + cert)");
    assert!(
        args[i_positions[1] + 1].ends_with("id_ed25519-cert.pub"),
        "le certificat doit être le second -i"
    );
}

/// Sans ssh_cert, un seul `-i` est présent.
#[test]
fn no_ssh_cert_single_identity_flag() {
    let mut s = base_server();
    s.ssh_key = "~/.ssh/id_ed25519".into();
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();

    let count = args.iter().filter(|a| *a == "-i").count();
    assert_eq!(count, 1, "un seul `-i` attendu sans certificat");
}

/// Un socket d'agent SSH personnalisé génère `-o IdentityAgent=...`.
#[test]
fn ssh_agent_sock_adds_identity_agent_option() {
    let mut s = base_server();
    s.ssh_agent_sock = "/run/user/1000/gnupg/S.gpg-agent.ssh".into();
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();

    let identity_agent_arg = args
        .windows(2)
        .find(|w| w[0] == "-o" && w[1].starts_with("IdentityAgent="))
        .expect("-o IdentityAgent=... attendu dans les args SSH");

    assert!(
        identity_agent_arg[1].contains("/run/user/1000/gnupg/S.gpg-agent.ssh"),
        "chemin de socket incorrect : {}",
        identity_agent_arg[1]
    );
}

/// Sans ssh_agent_sock, aucun `-o IdentityAgent` n'est ajouté.
#[test]
fn no_ssh_agent_sock_no_identity_agent_option() {
    let s = base_server();
    let args = build_ssh_args(&s, ConnectionMode::Direct, false).unwrap();

    let has_identity_agent = args
        .windows(2)
        .any(|w| w[0] == "-o" && w[1].starts_with("IdentityAgent="));
    assert!(
        !has_identity_agent,
        "IdentityAgent inattendu sans ssh_agent_sock"
    );
}
