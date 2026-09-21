use crate::config::{ConnectionMode, ResolvedServer, TunnelConfig};
use crate::ssh::client::build_ssh_args;
use anyhow::Result;
use std::process::{Child, Command};

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
                let reason = match exit.code() {
                    Some(0) => "terminé normalement".to_string(),
                    Some(c) => format!("code de sortie {}", c),
                    None => "tué par un signal".to_string(),
                };
                self.status = TunnelStatus::Dead(reason);
                self.child = None;
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

/// Construit les arguments SSH pour un tunnel `-L local:remote_host:remote_port -N`.
///
/// Réutilise [`build_ssh_args`] (invariant : destination toujours en dernière
/// position) en insérant les options de tunnel juste avant la destination.
///
/// **Non disponible** en mode [`ConnectionMode::Wallix`].
pub fn build_tunnel_args(
    server: &ResolvedServer,
    mode: ConnectionMode,
    tunnel: &TunnelConfig,
) -> Result<Vec<String>> {
    if mode == ConnectionMode::Wallix {
        anyhow::bail!("Les tunnels SSH ne sont pas disponibles en mode Wallix");
    }

    let mut args = build_ssh_args(server, mode, false)?;

    // Extraire la destination (invariant build_ssh_args : toujours dernière).
    let destination = args
        .pop()
        .ok_or_else(|| anyhow::anyhow!("liste d'args SSH vide"))?;

    // Options de tunnel insérées avant la destination.
    args.push("-N".into()); // pas de commande distante
    args.push("-L".into());
    args.push(format!(
        "{}:{}:{}",
        tunnel.local_port, tunnel.remote_host, tunnel.remote_port
    ));
    args.push("-o".into());
    args.push("ExitOnForwardFailure=yes".into());

    args.push(destination);

    Ok(args)
}

/// Lance un tunnel SSH en sous-processus non-bloquant.
///
/// stdin/stdout/stderr sont redirigés vers `/dev/null`. Le sous-processus survit
/// au thread appelant. Utiliser [`TunnelHandle::poll`] pour détecter sa fin,
/// et [`TunnelHandle::kill`] pour l'arrêter.
///
/// Retourne une erreur si le mode est [`ConnectionMode::Wallix`] ou si `ssh`
/// ne peut pas être lancé.
pub fn spawn_tunnel(
    server: &ResolvedServer,
    mode: ConnectionMode,
    config: TunnelConfig,
    yaml_index: Option<usize>,
    user_idx: usize,
) -> Result<TunnelHandle> {
    let args = build_tunnel_args(server, mode, &config)?;
    let child = Command::new("ssh")
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| anyhow::anyhow!("Impossible de lancer le tunnel SSH : {}", e))?;

    Ok(TunnelHandle {
        config,
        yaml_index,
        user_idx,
        status: TunnelStatus::Running,
        child: Some(child),
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

    #[test]
    fn tunnel_args_wallix_rejected() {
        let s = base_server();
        let t = pg_tunnel();
        let result = build_tunnel_args(&s, ConnectionMode::Wallix, &t);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Wallix"));
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
