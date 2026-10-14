use crate::config::{ConnectionMode, ResolvedServer, TunnelConfig};
use crate::ssh::client::build_ssh_args;
use anyhow::Result;
use std::process::{Child, ChildStdin, Command, Stdio};

// ─── Types ────────────────────────────────────────────────────────────────────

/// État courant d'un tunnel SSH.
#[derive(Debug)]
pub enum TunnelStatus {
    /// Tunnel non démarré.
    Idle,
    /// Tunnel actif — le sous-processus SSH tourne.
    Running,
    /// Le processus SSH s'est terminé inopinément.
    Dead(String),
    /// Erreur au démarrage.
    Error(String),
}

/// Instance d'un tunnel SSH géré par l'application.
///
/// Encapsule la configuration effective, l'identité dans la liste overrides,
/// le statut observable depuis la TUI, et le sous-processus SSH sous-jacent.
pub struct TunnelHandle {
    /// Configuration effective du tunnel.
    pub config: TunnelConfig,
    /// Index dans la liste YAML du serveur (`None` = tunnel ajouté manuellement).
    pub yaml_index: Option<usize>,
    /// Index dans la liste d'[`effective_tunnels`] du serveur.
    pub user_idx: usize,
    /// État observable du tunnel.
    pub status: TunnelStatus,
    /// Sous-processus SSH — présent uniquement quand `status == Running`.
    child: Option<Child>,
    /// Entrée de la session shell (Wallix) gardée ouverte : le shell distant
    /// attend des commandes au lieu de se terminer sur EOF.
    session_stdin: Option<ChildStdin>,
}

impl std::fmt::Debug for TunnelHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TunnelHandle")
            .field("config", &self.config)
            .field("yaml_index", &self.yaml_index)
            .field("user_idx", &self.user_idx)
            .field("status", &self.status)
            .field("child_pid", &self.child.as_ref().map(|c| c.id()))
            .finish()
    }
}

// ─── Impl TunnelHandle ────────────────────────────────────────────────────────

impl TunnelHandle {
    /// Crée un handle en état `Idle` (aucun processus lancé).
    pub fn new(config: TunnelConfig, yaml_index: Option<usize>, user_idx: usize) -> Self {
        Self {
            config,
            yaml_index,
            user_idx,
            status: TunnelStatus::Idle,
            child: None,
            session_stdin: None,
        }
    }

    /// Retourne `true` si le tunnel est actuellement en cours d'exécution.
    pub fn is_running(&self) -> bool {
        matches!(self.status, TunnelStatus::Running)
    }

    /// Sonde si le sous-processus est toujours vivant (non-bloquant).
    ///
    /// Retourne `true` si le statut vient de passer à [`TunnelStatus::Dead`].
    pub fn poll(&mut self) -> bool {
        let Some(child) = &mut self.child else {
            return false;
        };
        match child.try_wait() {
            Ok(Some(exit)) => {
                let mut reason = match exit.code() {
                    Some(0) => "terminé normalement".to_string(),
                    Some(c) => format!("code de sortie {}", c),
                    None => "tué par un signal".to_string(),
                };
                // Sortie d'erreur de ssh (ex. « Cible invalide » renvoyé par
                // Wallix, port local occupé…) : seul indice exploitable.
                if let Some(mut err) = child.stderr.take() {
                    use std::io::Read as _;
                    let mut stderr = String::new();
                    let _ = err.read_to_string(&mut stderr);
                    let detail = stderr
                        .lines()
                        .map(str::trim)
                        .filter(|l| !l.is_empty() && !l.starts_with("**"))
                        .collect::<Vec<_>>()
                        .join(" | ");
                    if !detail.is_empty() {
                        reason = format!("{reason} : {detail}");
                    }
                }
                self.status = TunnelStatus::Dead(reason);
                self.child = None;
                self.session_stdin = None;
                true
            }
            Ok(None) => false, // toujours en cours
            Err(e) => {
                self.status = TunnelStatus::Dead(e.to_string());
                self.child = None;
                true
            }
        }
    }

    /// Arrête le tunnel proprement (SIGTERM → wait). Le statut revient à `Idle`.
    pub fn kill(&mut self) {
        self.session_stdin = None;
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.status = TunnelStatus::Idle;
    }
}

impl Drop for TunnelHandle {
    fn drop(&mut self) {
        self.kill();
    }
}

// ─── Fonctions publiques ──────────────────────────────────────────────────────

/// Construit les arguments SSH pour un tunnel `-L local:remote_host:remote_port`.
///
/// Réutilise [`build_ssh_args`] (invariant : destination toujours en dernière
/// position) en insérant les options de tunnel juste avant la destination.
///
/// En mode [`ConnectionMode::Wallix`] :
/// - l'autorisation doit être connue par la config (login direct
///   `user@cible+SSH+AUTH+bastion_user`) : sinon Wallix affiche son menu, ce
///   qui est impossible pour un tunnel en arrière-plan ;
/// - une session shell sans PTY (`-T`) remplace `-N` : Wallix coupe la
///   connexion si le forwarding est demandé hors session ouverte sur la cible ;
/// - une destination locale (`127.0.0.1`, `localhost`, `::1`) est remplacée
///   par le FQDN du serveur, comme le documente le bastion.
pub fn build_tunnel_args(
    server: &ResolvedServer,
    mode: ConnectionMode,
    tunnel: &TunnelConfig,
) -> Result<Vec<String>> {
    let wallix = mode == ConnectionMode::Wallix;
    if wallix && !wallix_tunnel_available(server) {
        anyhow::bail!(
            "Tunnel Wallix impossible sans autorisation connue : déclarez-la dans wallix.authorizations"
        );
    }

    let mut args = build_ssh_args(server, mode, false)?;

    let target_host = server
        .host
        .split_once(':')
        .map_or(server.host.as_str(), |(h, _)| h);
    let remote_host = if wallix
        && matches!(
            tunnel.remote_host.as_str(),
            "127.0.0.1" | "localhost" | "::1"
        ) {
        target_host
    } else {
        tunnel.remote_host.as_str()
    };

    // Extraire la destination (invariant build_ssh_args : toujours dernière).
    let destination = args
        .pop()
        .ok_or_else(|| anyhow::anyhow!("liste d'args SSH vide"))?;

    // Options de tunnel insérées avant la destination.
    // Wallix : session shell (sans PTY) maintenue ouverte ; sinon pas de session.
    args.push(if wallix { "-T" } else { "-N" }.into());
    args.push("-L".into());
    args.push(format!(
        "{}:{}:{}",
        tunnel.local_port, remote_host, tunnel.remote_port
    ));
    args.push("-o".into());
    args.push("ExitOnForwardFailure=yes".into());

    args.push(destination);

    Ok(args)
}

/// Un tunnel via Wallix exige une autorisation unique connue par la config.
pub fn wallix_tunnel_available(server: &ResolvedServer) -> bool {
    crate::wallix::direct_authorization(server).is_some()
}

/// Lance `program` en arrière-plan : stdout vers `/dev/null`, stderr conservé
/// (raison d'un arrêt, voir [`TunnelHandle::poll`]).
///
/// Avec `keep_stdin_open`, stdin est un tube jamais écrit, retourné à
/// l'appelant : une session shell distante reste ouverte tant qu'il est gardé.
fn spawn_tunnel_process(
    program: &str,
    args: &[String],
    keep_stdin_open: bool,
) -> Result<(Child, Option<ChildStdin>)> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(if keep_stdin_open {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("Impossible de lancer le tunnel SSH : {}", e))?;
    let stdin = child.stdin.take();
    Ok((child, stdin))
}

/// Lance un tunnel SSH en sous-processus non-bloquant.
///
/// Le sous-processus survit au thread appelant. Utiliser [`TunnelHandle::poll`]
/// pour détecter sa fin, et [`TunnelHandle::kill`] pour l'arrêter.
///
/// Retourne une erreur si le tunnel n'est pas disponible (Wallix sans
/// autorisation connue) ou si `ssh` ne peut pas être lancé.
pub fn spawn_tunnel(
    server: &ResolvedServer,
    mode: ConnectionMode,
    config: TunnelConfig,
    yaml_index: Option<usize>,
    user_idx: usize,
) -> Result<TunnelHandle> {
    let args = build_tunnel_args(server, mode, &config)?;
    let (child, session_stdin) =
        spawn_tunnel_process("ssh", &args, mode == ConnectionMode::Wallix)?;

    Ok(TunnelHandle {
        config,
        yaml_index,
        user_idx,
        status: TunnelStatus::Running,
        child: Some(child),
        session_stdin,
    })
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
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

    fn pg_tunnel() -> TunnelConfig {
        TunnelConfig {
            local_port: 5433,
            remote_host: "127.0.0.1".into(),
            remote_port: 5432,
            label: "PostgreSQL".into(),
        }
    }

    #[test]
    fn tunnel_args_direct_contains_n_and_l() {
        let s = base_server();
        let t = pg_tunnel();
        let args = build_tunnel_args(&s, ConnectionMode::Direct, &t).unwrap();

        assert!(args.contains(&"-N".to_string()), "doit contenir -N");

        let l_idx = args.iter().position(|a| a == "-L").expect("-L absent");
        assert_eq!(args[l_idx + 1], "5433:127.0.0.1:5432");

        // La destination reste la dernière.
        assert_eq!(args.last().unwrap(), "admin@198.51.100.1");
        // ExitOnForwardFailure doit être présent.
        assert!(args.iter().any(|a| a.contains("ExitOnForwardFailure")));
    }

    #[test]
    fn tunnel_args_jump_keeps_j_flag() {
        let mut s = base_server();
        s.jump_host = Some("jump.example.com".into());
        let t = pg_tunnel();
        let args = build_tunnel_args(&s, ConnectionMode::Jump, &t).unwrap();
        assert!(args.contains(&"-J".to_string()), "doit contenir -J");
        assert!(args.contains(&"-N".to_string()), "-N absent");
        // La destination reste la dernière.
        assert_eq!(args.last().unwrap(), "admin@198.51.100.1");
    }

    /// Sans autorisation connue, Wallix affiche son menu (plusieurs
    /// autorisations possibles) : impossible pour un tunnel en arrière-plan.
    #[test]
    fn tunnel_args_wallix_without_authorization_rejected() {
        let mut s = base_server();
        s.bastion_host = Some("bastion.example.com".into());
        let err = build_tunnel_args(&s, ConnectionMode::Wallix, &pg_tunnel())
            .unwrap_err()
            .to_string();
        assert!(err.contains("wallix.authorizations"), "{err}");
    }

    /// Autorisation connue → login direct, et session shell sans PTY (`-T`)
    /// au lieu de `-N` : Wallix coupe un forwarding demandé hors session.
    #[test]
    fn tunnel_args_wallix_with_authorization() {
        let mut s = base_server();
        s.host = "srv01.example.fr".into();
        s.bastion_host = Some("bastion.example.com".into());
        s.bastion_user = Some("buser".into());
        s.wallix_authorizations = vec!["PR-APP_admins".into()];
        let args = build_tunnel_args(&s, ConnectionMode::Wallix, &pg_tunnel()).unwrap();
        let l_pos = args.iter().position(|a| a == "-l").expect("-l present");
        assert_eq!(
            args[l_pos + 1],
            "admin@srv01.example.fr+SSH+PR-APP_admins+buser"
        );
        assert!(args.contains(&"-A".to_string()), "{args:?}");
        assert!(args.contains(&"-T".to_string()), "{args:?}");
        assert!(!args.contains(&"-N".to_string()), "{args:?}");
        assert_eq!(args.last().unwrap(), "bastion.example.com");
    }

    /// Hors Wallix, tunnel sans session (`-N`).
    #[test]
    fn tunnel_args_direct_uses_no_session() {
        let args = build_tunnel_args(&base_server(), ConnectionMode::Direct, &pg_tunnel()).unwrap();
        assert!(args.contains(&"-N".to_string()), "{args:?}");
        assert!(!args.contains(&"-T".to_string()), "{args:?}");
    }

    /// La session Wallix est un shell lisant stdin : tant que le handle garde
    /// stdin ouvert, le processus reste vivant ; sans cela il s'arrête (EOF).
    #[test]
    #[cfg(unix)]
    fn tunnel_process_kept_alive_by_open_stdin() {
        let args = vec!["-c".to_string(), "cat >/dev/null".to_string()];

        let (mut child, stdin) = spawn_tunnel_process("sh", &args, true).unwrap();
        assert!(stdin.is_some());
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(child.try_wait().unwrap().is_none(), "doit rester vivant");
        drop(stdin);
        assert!(child.wait().unwrap().success(), "EOF → fin du processus");

        let (mut child, stdin) = spawn_tunnel_process("sh", &args, false).unwrap();
        assert!(stdin.is_none());
        assert!(child.wait().unwrap().success());
    }

    /// En Wallix, c'est le bastion qui ouvre la connexion : une destination
    /// locale (`127.0.0.1`, `localhost`, `::1`) désigne en fait le serveur.
    #[test]
    fn tunnel_args_wallix_loopback_becomes_server_fqdn() {
        let mut s = base_server();
        s.host = "srv01.example.fr:2222".into();
        s.bastion_host = Some("bastion.example.com".into());
        s.wallix_authorizations = vec!["PR-APP_admins".into()];
        for loopback in ["127.0.0.1", "localhost", "::1"] {
            let mut t = pg_tunnel();
            t.remote_host = loopback.into();
            let args = build_tunnel_args(&s, ConnectionMode::Wallix, &t).unwrap();
            assert!(
                args.contains(&"5433:srv01.example.fr:5432".to_string()),
                "{loopback}: {args:?}"
            );
        }
        // Destination explicite : conservée telle quelle.
        let mut t = pg_tunnel();
        t.remote_host = "db.example.fr".into();
        let args = build_tunnel_args(&s, ConnectionMode::Wallix, &t).unwrap();
        assert!(
            args.contains(&"5433:db.example.fr:5432".to_string()),
            "{args:?}"
        );
    }

    /// Hors Wallix, `127.0.0.1` reste relatif au serveur SSH (inchangé).
    #[test]
    fn tunnel_args_direct_keeps_loopback() {
        let args = build_tunnel_args(&base_server(), ConnectionMode::Direct, &pg_tunnel()).unwrap();
        assert!(
            args.contains(&"5433:127.0.0.1:5432".to_string()),
            "{args:?}"
        );
    }

    /// Un tunnel mort expose la sortie d'erreur de ssh (ex. refus du bastion).
    #[test]
    #[cfg(unix)]
    fn tunnel_poll_dead_reports_stderr() {
        let child = Command::new("sh")
            .args(["-c", "echo 'Cible invalide' >&2; exit 255"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut h = TunnelHandle::new(pg_tunnel(), None, 0);
        h.status = TunnelStatus::Running;
        h.child = Some(child);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !h.poll() {
            assert!(
                std::time::Instant::now() < deadline,
                "le processus ne s'est pas terminé"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        match &h.status {
            TunnelStatus::Dead(reason) => {
                assert!(reason.contains("255"), "{reason}");
                assert!(reason.contains("Cible invalide"), "{reason}");
            }
            other => panic!("attendu Dead, obtenu {other:?}"),
        }
    }

    #[test]
    fn tunnel_args_destination_last_invariant() {
        // Avec clé SSH et option supplémentaire, la destination reste bien dernière.
        let mut s = base_server();
        s.ssh_key = "~/.ssh/id_ed25519".into();
        s.ssh_options = vec!["ServerAliveInterval=30".into()];
        let t = pg_tunnel();
        let args = build_tunnel_args(&s, ConnectionMode::Direct, &t).unwrap();
        assert_eq!(args.last().unwrap(), "admin@198.51.100.1");
    }

    #[test]
    fn tunnel_handle_idle_by_default() {
        let t = pg_tunnel();
        let h = TunnelHandle::new(t, Some(0), 0);
        assert!(!h.is_running());
        assert!(matches!(h.status, TunnelStatus::Idle));
    }

    #[test]
    fn tunnel_handle_poll_returns_false_when_idle() {
        let t = pg_tunnel();
        let mut h = TunnelHandle::new(t, None, 0);
        assert!(!h.poll(), "poll sur Idle doit retourner false");
    }

    #[test]
    fn tunnel_args_includes_ssh_key() {
        let mut s = base_server();
        s.ssh_key = "/home/user/.ssh/id_ed25519".into();
        let t = pg_tunnel();
        let args = build_tunnel_args(&s, ConnectionMode::Direct, &t).unwrap();
        let i_pos = args.iter().position(|a| a == "-i").expect("-i absent");
        assert_eq!(args[i_pos + 1], "/home/user/.ssh/id_ed25519");
        // Destination toujours en dernière position
        assert_eq!(args.last().unwrap(), "admin@198.51.100.1");
    }

    #[test]
    fn tunnel_args_ssh_options_before_destination() {
        let mut s = base_server();
        s.ssh_options = vec!["ServerAliveInterval=30".into()];
        let t = pg_tunnel();
        let args = build_tunnel_args(&s, ConnectionMode::Direct, &t).unwrap();
        let dest_pos = args
            .iter()
            .rposition(|a| a == "admin@198.51.100.1")
            .unwrap();
        let opt_pos = args
            .iter()
            .position(|a| a == "ServerAliveInterval=30")
            .unwrap();
        assert!(
            opt_pos < dest_pos,
            "les options SSH doivent précéder la destination"
        );
    }

    #[test]
    fn tunnel_args_f_dev_null_when_not_using_system_config() {
        let s = base_server(); // use_system_ssh_config = false
        let t = pg_tunnel();
        let args = build_tunnel_args(&s, ConnectionMode::Direct, &t).unwrap();
        let f_pos = args.iter().position(|a| a == "-F").expect("-F absent");
        assert_eq!(args[f_pos + 1], "/dev/null");
    }

    #[test]
    fn tunnel_args_no_f_flag_with_system_config() {
        let mut s = base_server();
        s.use_system_ssh_config = true;
        let t = pg_tunnel();
        let args = build_tunnel_args(&s, ConnectionMode::Direct, &t).unwrap();
        assert!(
            !args.contains(&"-F".to_string()),
            "-F ne doit pas être présent quand use_system_ssh_config=true"
        );
    }

    #[test]
    fn tunnel_l_flag_format() {
        // Vérifie le format exact de -L localPort:remoteHost:remotePort
        let t = TunnelConfig {
            local_port: 15432,
            remote_host: "db.internal".into(),
            remote_port: 5432,
            label: "Test".into(),
        };
        let args = build_tunnel_args(&base_server(), ConnectionMode::Direct, &t).unwrap();
        let l_pos = args.iter().position(|a| a == "-L").expect("-L absent");
        assert_eq!(args[l_pos + 1], "15432:db.internal:5432");
    }
}
