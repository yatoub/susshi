//! Transfert de fichiers SFTP natif via `libssh2`.
//!
//! Ce module expose [également les types partagés `ScpDirection` et `ScpEvent`]
//! précédemment dans `ssh::scp`.
//!
//! ## Interface publique
//!
//! [`spawn_sftp`] retourne un [`std::sync::mpsc::Receiver<ScpEvent>`].
//!
//! ## Avantages vs spawn `scp`
//!
//! - Progression précise (taille du fichier connue dès l'ouverture).
//! - Aucune dépendance sur l'utilitaire `scp` installé (OpenSSH ≥ 9.0 le déprécie).
//! - Gestion propre des erreurs SFTP (codes d'état standardisés).
//! - Compatible avec les serveurs SFTP-only (sans shell).
//!
//! ## Authentification (par priorité)
//!
//! 1. Agent SSH (`$SSH_AUTH_SOCK`)
//! 2. Clé explicite dans `server.ssh_key` (tilde expandé)
//! 3. Clés par défaut : `~/.ssh/id_ed25519`, `~/.ssh/id_rsa`, `~/.ssh/id_ecdsa`
//!
//! ## Modes supportés
//!
//! | Mode     | Support |
//! |----------|---------|
//! | Direct   | ✔ session SSH2 directe |
//! | Jump     | ✔ `ssh -W` comme `scp -J` — un seul chiffrement |
//! | Wallix   | ✔ binaire `scp` système (voir ci-dessous) |
//!
//! En mode Wallix, le transfert délègue au binaire `scp` d'OpenSSH avec le
//! login `user@cible+SSH+autorisation+bastion_user@bastion` (aucun menu
//! possible : autorisations de la config essayées dans l'ordre, détection en
//! dernier recours) et le transfert
//! d'agent (`-A`) :
//! le bastion s'authentifie sur la cible via l'agent transféré, ce que
//! libssh2 ne sait pas faire. `scp` tourne sur un pseudo-terminal : son
//! progress meter alimente la barre de progression.

use crate::config::{ConnectionMode, ResolvedServer};
use anyhow::Result;
use std::sync::mpsc;

// ─── Types partagés ────────────────────────────────────────────────────────────

/// Sens du transfert SFTP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScpDirection {
    /// Envoi : local → serveur.
    Upload,
    /// Récupération : serveur → local.
    Download,
}

impl ScpDirection {
    /// Libellé court en français.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Upload => "Upload",
            Self::Download => "Download",
        }
    }
}

/// Évènement émis par le thread de transfert SFTP.
#[derive(Debug)]
pub enum ScpEvent {
    /// Taille totale du fichier (envoyé une seule fois à l'ouverture).
    FileSize(u64),
    /// Progression (0–100).
    Progress(u8),
    /// Transfert terminé — `true` = succès.
    Done(bool),
    /// Erreur irrécupérable.
    Error(String),
}

// ─── API publique ─────────────────────────────────────────────────────────────

/// Lance un transfert SFTP en arrière-plan dans un thread dédié.
///
/// Retourne immédiatement un [`mpsc::Receiver<ScpEvent>`] qui émettra :
/// - [`ScpEvent::Progress`] (0–100) à chaque chunk de 64 KiB transféré
/// - [`ScpEvent::Done`] à la fin (succès)
/// - [`ScpEvent::Error`] si une erreur irrécupérable survient
///
/// Contrairement à `spawn_scp`, aucun PID n'est retourné (pas de sous-processus).
/// Le thread SFTP peut être abandonné en droppant le `Receiver` — il détectera
/// `SendError` et se terminera proprement.
#[cfg(unix)]
pub fn spawn_sftp(
    server: &ResolvedServer,
    mode: ConnectionMode,
    direction: ScpDirection,
    local: &str,
    remote: &str,
) -> Result<mpsc::Receiver<ScpEvent>> {
    let server = server.clone();
    let local = shellexpand::tilde(local).into_owned();
    let remote = remote.to_string();

    let (tx, rx) = mpsc::channel::<ScpEvent>();

    std::thread::spawn(move || {
        let result = if mode == ConnectionMode::Wallix {
            transfer_wallix_scp(&server, &direction, &local, &remote, &tx)
        } else {
            transfer_inner(&server, mode, &direction, &local, &remote, &tx)
        };
        match result {
            Ok(()) => {
                let _ = tx.send(ScpEvent::Done(true));
            }
            Err(e) => {
                let _ = tx.send(ScpEvent::Error(format!("{e:#}")));
            }
        }
    });

    Ok(rx)
}

/// Stub non-Unix — SFTP natif non disponible.
#[cfg(not(unix))]
pub fn spawn_sftp(
    _server: &ResolvedServer,
    _mode: ConnectionMode,
    _direction: ScpDirection,
    _local: &str,
    _remote: &str,
) -> Result<mpsc::Receiver<ScpEvent>> {
    anyhow::bail!("SFTP non disponible sur cette plateforme")
}

// ─── Implémentation Unix ──────────────────────────────────────────────────────

#[cfg(unix)]
fn transfer_inner(
    server: &ResolvedServer,
    mode: ConnectionMode,
    direction: &ScpDirection,
    local: &str,
    remote: &str,
    tx: &mpsc::Sender<ScpEvent>,
) -> Result<()> {
    use anyhow::Context;
    use std::fs::File;
    use std::path::{Path, PathBuf};

    // ── 1. Ouvre la session SSH ───────────────────────────────────────────────
    let sess = open_session(server, mode)?;

    // ── 2. Résolution du chemin distant via SFTP (stat + realpath) ───────────
    //
    // SFTP est utilisé UNIQUEMENT pour la résolution de chemin.
    // Le transfert lui-même utilise le protocole SCP (streaming pur, sans
    // request/ACK par paquet) pour éviter la limitation MAX_SFTP_OUTGOING_SIZE
    // de libssh2 (30 KB), qui plafonne le débit à ~1–3 MB/s sur liens à latence.
    let remote_path: PathBuf = {
        let sftp = sess
            .sftp()
            .map_err(|e| anyhow::anyhow!("ouverture du canal SFTP : {e}"))?;
        let raw = resolve_remote_path(&sftp, remote);

        if remote.ends_with('/') || remote.ends_with('\\') || raw.as_os_str().is_empty() {
            let filename = Path::new(local)
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("transfer"));
            raw.join(filename)
        } else {
            match sftp.stat(&raw) {
                Ok(stat) if stat.is_dir() => {
                    let filename = Path::new(local)
                        .file_name()
                        .unwrap_or_else(|| std::ffi::OsStr::new("transfer"));
                    raw.join(filename)
                }
                _ => raw,
            }
        }
        // `sftp` est droppé ici — le canal SFTP est fermé avant d'ouvrir SCP.
    };

    // ── 3. Transfert SCP (streaming, fenêtre SSH classique, plein débit) ─────
    match direction {
        ScpDirection::Upload => {
            let local_path = Path::new(local);
            let file_size = std::fs::metadata(local_path).map(|m| m.len()).unwrap_or(0);
            let _ = tx.send(ScpEvent::FileSize(file_size));
            let mut src = File::open(local_path)?;
            let mut channel = sess
                .scp_send(&remote_path, 0o644, file_size, None)
                .map_err(|e| {
                    anyhow::anyhow!("ouverture SCP (upload) {}: {e}", remote_path.display())
                })?;
            copy_with_progress(&mut src, &mut channel, file_size, tx)?;
            // Fin de fichier SCP : libssh2 n'envoie pas l'octet nul final ni ne
            // lit l'acquittement. OpenSSH le tolère, mais un proxy (Wallix)
            // abandonne le fichier sans cet octet — et l'erreur serait muette.
            {
                use std::io::Write as _;
                channel.write_all(&[0])?;
                channel.flush()?;
            }
            read_scp_ack(&mut channel)
                .with_context(|| format!("écriture distante de {}", remote_path.display()))?;
            // Fermeture propre du canal SCP (envoi du signal EOF au serveur).
            channel.send_eof()?;
            channel.wait_eof()?;
            channel.close()?;
            channel.wait_close()?;
        }
        ScpDirection::Download => {
            let local_path = {
                let p = Path::new(local);
                if local.ends_with('/') || local.ends_with('\\') || p.is_dir() {
                    let filename = remote_path
                        .file_name()
                        .unwrap_or_else(|| std::ffi::OsStr::new("download"));
                    p.join(filename)
                } else {
                    p.to_path_buf()
                }
            };
            // scp_recv fournit directement la taille du fichier dans le header SCP.
            let (mut channel, scp_stat) = sess.scp_recv(&remote_path).map_err(|e| {
                anyhow::anyhow!("ouverture SCP (download) {}: {e}", remote_path.display())
            })?;
            let file_size = scp_stat.size();
            let _ = tx.send(ScpEvent::FileSize(file_size));
            let mut dst = File::create(&local_path)?;
            copy_with_progress(&mut channel, &mut dst, file_size, tx)?;
        }
    }

    Ok(())
}

/// Ouvre une session SSH authentifiée selon le mode de connexion.
#[cfg(unix)]
fn open_session(server: &ResolvedServer, mode: ConnectionMode) -> Result<ssh2::Session> {
    use std::net::TcpStream;

    match mode {
        ConnectionMode::Direct => {
            let (host, port) = resolve_host_port(server);
            let tcp = TcpStream::connect(format!("{}:{}", host, port))?;
            let mut sess = ssh2::Session::new()?;
            sess.set_tcp_stream(tcp);
            sess.handshake()?;
            auth_session(&mut sess, &server.user, &server.ssh_key)?;
            Ok(sess)
        }
        ConnectionMode::Jump => {
            let jump_str = server
                .jump_host
                .as_deref()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| anyhow::anyhow!("Jump host non configuré pour ce serveur"))?;
            open_session_via_jump(jump_str, server)
        }
        // Le mode Wallix passe par `transfer_wallix_scp` (binaire `scp`).
        ConnectionMode::Wallix => anyhow::bail!("session libssh2 non utilisée en mode Wallix"),
    }
}

/// Construit les arguments du binaire `scp` pour un transfert via Wallix.
///
/// Format du login : `user@cible+SSH+autorisation+bastion_user@bastion:chemin`, avec `-A` pour
/// que le bastion puisse s'authentifier sur la cible via l'agent transféré.
/// `BatchMode=yes` : aucun prompt possible (pas de TTY), échec immédiat.
#[cfg(unix)]
fn build_wallix_scp_args(
    server: &ResolvedServer,
    direction: &ScpDirection,
    local: &str,
    remote: &str,
    authorization: Option<&str>,
) -> Result<Vec<String>> {
    let bastion = server
        .bastion_host
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("Wallix host not configured for this server"))?;
    let (bastion_host, bastion_port) = match bastion.split_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (bastion, None),
    };
    let mut args: Vec<String> = Vec::new();
    if !server.use_system_ssh_config {
        args.extend(["-F".into(), "/dev/null".into()]);
    }
    args.push("-A".into());
    for key in [&server.ssh_key, &server.ssh_cert] {
        if !key.is_empty() {
            args.push("-i".into());
            args.push(shellexpand::tilde(key).into_owned());
        }
    }
    for opt in &server.ssh_options {
        if opt.starts_with('-') {
            args.push(opt.clone());
        } else {
            args.extend(["-o".into(), opt.clone()]);
        }
    }
    if !server.use_system_ssh_config
        && !server
            .ssh_options
            .iter()
            .any(|o| o.to_ascii_lowercase().contains("stricthostkeychecking"))
    {
        args.extend(["-o".into(), "StrictHostKeyChecking=accept-new".into()]);
    }
    if !server.ssh_agent_sock.is_empty() {
        let sock = shellexpand::tilde(&server.ssh_agent_sock).into_owned();
        args.extend(["-o".into(), format!("IdentityAgent={sock}")]);
    }
    // BatchMode : aucun prompt possible (pas de TTY). LogLevel=ERROR : masque
    // les avertissements (known_hosts, post-quantique) qui noient l'erreur.
    args.extend(["-o".into(), "BatchMode=yes".into()]);
    args.extend(["-o".into(), "LogLevel=ERROR".into()]);
    if let Some(p) = bastion_port {
        args.extend(["-P".into(), p.to_string()]);
    }

    // Login Wallix direct : user@cible+PROTOCOLE[+autorisation]+bastion_user.
    let login = crate::wallix::build_direct_login(server, authorization);
    let endpoint = format!("{login}@{bastion_host}:{remote}");
    match direction {
        ScpDirection::Upload => args.extend([local.to_string(), endpoint]),
        ScpDirection::Download => args.extend([endpoint, local.to_string()]),
    }
    Ok(args)
}

/// Résume la sortie d'erreur de `scp` sur une ligne, sans les avertissements
/// d'OpenSSH (known_hosts, post-quantique). Si tout est filtré, renvoie la
/// sortie brute pour ne rien perdre.
#[cfg(unix)]
fn scp_stderr_summary(stderr: &str) -> String {
    let all: Vec<&str> = stderr
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let useful: Vec<&str> = all
        .iter()
        .copied()
        .filter(|l| !l.starts_with("Warning: Permanently added") && !l.starts_with("**"))
        .collect();
    if useful.is_empty() {
        all.join(" | ")
    } else {
        useful.join(" | ")
    }
}

/// Extrait le pourcentage du progress meter d'OpenSSH
/// (`fichier   42%  420MB  12.3MB/s   00:47 ETA`, rafraîchi par `\r`).
///
/// Les octets reçus peuvent couper une ligne n'importe où : la fin non
/// terminée est conservée pour le prochain appel.
#[cfg(unix)]
#[derive(Default)]
struct ScpProgressParser {
    pending: String,
}

#[cfg(unix)]
impl ScpProgressParser {
    /// Borne du tampon si la sortie ne contient jamais de fin de ligne.
    const MAX_PENDING: usize = 4096;

    /// Ajoute `bytes` et renvoie le dernier pourcentage lisible, s'il y en a un.
    fn feed(&mut self, bytes: &[u8]) -> Option<u8> {
        self.pending.push_str(&String::from_utf8_lossy(bytes));
        let text = std::mem::take(&mut self.pending);
        let mut latest = None;
        let mut segments = text.split(['\r', '\n']).peekable();
        while let Some(segment) = segments.next() {
            if let Some(pct) = Self::percent_in(segment) {
                latest = Some(pct);
            }
            if segments.peek().is_none() && segment.len() <= Self::MAX_PENDING {
                self.pending = segment.to_string();
            }
        }
        latest
    }

    /// Dernier jeton `N%` (0–100) de la ligne, hors dernier jeton : dans la
    /// vraie ligne, le pourcentage est toujours suivi de la taille et du
    /// débit, ce qui écarte un nom de fichier ou une ligne encore tronquée.
    fn percent_in(segment: &str) -> Option<u8> {
        let tokens: Vec<&str> = segment.split_whitespace().collect();
        let candidates = &tokens[..tokens.len().saturating_sub(1)];
        candidates.iter().rev().find_map(|t| {
            t.strip_suffix('%')
                .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
                .and_then(|n| n.parse::<u8>().ok())
                .filter(|p| *p <= 100)
        })
    }
}

/// Lance `cmd` avec sa sortie standard sur un pseudo-terminal dont il est le
/// processus de premier plan — condition pour que `scp` affiche son progress
/// meter — et relaie les pourcentages lus en [`ScpEvent::Progress`].
///
/// La sortie d'erreur reste un pipe, renvoyée avec le statut de sortie.
/// Le processus est tué si le `Receiver` est droppé (transfert annulé côté TUI).
#[cfg(unix)]
fn run_with_progress_pty(
    mut cmd: std::process::Command,
    tx: &mpsc::Sender<ScpEvent>,
) -> Result<(std::process::ExitStatus, String)> {
    use std::io::Read as _;
    use std::os::unix::io::AsRawFd;
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;

    // Largeur confortable : le progress meter tronque le nom de fichier, pas
    // les colonnes chiffrées.
    let winsize = nix::pty::Winsize {
        ws_row: 24,
        ws_col: 200,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let pty = nix::pty::openpty(Some(&winsize), None)
        .map_err(|e| anyhow::anyhow!("création du pty pour scp : {e}"))?;

    cmd.stdin(Stdio::null())
        .stdout(Stdio::from(pty.slave))
        .stderr(Stdio::piped());
    // Nouvelle session dont le pty devient le terminal de contrôle : scp
    // vérifie `getpgrp() == tcgetpgrp(stdout)` avant d'afficher sa progression.
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() < 0 || libc::ioctl(libc::STDOUT_FILENO, libc::TIOCSCTTY as _, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| anyhow::anyhow!("lancement de scp : {e}"))?;
    // Ferme la copie de l'esclave détenue par `cmd` : sans elle, la lecture
    // du maître ne verrait jamais la fin de sortie.
    drop(cmd);

    let mut master = std::fs::File::from(pty.master);
    let master_fd = master.as_raw_fd();
    let mut parser = ScpProgressParser::default();
    let mut buf = [0u8; 4096];
    let mut progress: u8 = 0;
    let mut master_open = true;

    let status = loop {
        if master_open {
            let mut pollfd = libc::pollfd {
                fd: master_fd,
                events: libc::POLLIN,
                revents: 0,
            };
            let rc = unsafe { libc::poll(&mut pollfd, 1, 200) };
            if rc > 0 {
                if pollfd.revents & libc::POLLIN != 0 {
                    match master.read(&mut buf) {
                        // EIO (Linux) quand plus aucun processus ne tient l'esclave.
                        Ok(0) | Err(_) => master_open = false,
                        Ok(n) => {
                            if let Some(pct) = parser.feed(&buf[..n]) {
                                progress = pct;
                            }
                        }
                    }
                } else if pollfd.revents & (libc::POLLHUP | libc::POLLERR) != 0 {
                    master_open = false;
                }
            }
        } else {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }

        if let Some(status) = child.try_wait()? {
            break status;
        }
        // Sert aussi de sonde d'annulation : échoue si la TUI a abandonné.
        if tx.send(ScpEvent::Progress(progress)).is_err() {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("transfert annulé");
        }
    };

    let mut stderr = String::new();
    if let Some(mut err) = child.stderr.take() {
        let _ = err.read_to_string(&mut stderr);
    }
    Ok((status, stderr))
}

/// Transfert via Wallix en déléguant au binaire `scp` système.
///
/// `scp` tourne sur un pseudo-terminal pour que son progress meter alimente
/// la barre de progression. Le processus est tué si le `Receiver` est droppé
/// (transfert annulé côté TUI).
#[cfg(unix)]
fn transfer_wallix_scp(
    server: &ResolvedServer,
    direction: &ScpDirection,
    local: &str,
    remote: &str,
    tx: &mpsc::Sender<ScpEvent>,
) -> Result<()> {
    use std::process::Command;

    if *direction == ScpDirection::Upload {
        let size = std::fs::metadata(local).map(|m| m.len()).unwrap_or(0);
        let _ = tx.send(ScpEvent::FileSize(size));
    }

    // Chaque autorisation candidate est essayée tant que Wallix répond
    // « Cible invalide » ; toute autre erreur est remontée immédiatement.
    let candidates = crate::wallix::fallback_authorizations(server);
    let mut refused: Vec<String> = Vec::new();
    for authorization in &candidates {
        let args =
            build_wallix_scp_args(server, direction, local, remote, authorization.as_deref())?;
        let n = args.len();
        let endpoint = match direction {
            ScpDirection::Upload => args[n - 1].clone(),
            ScpDirection::Download => args[n - 2].clone(),
        };

        let mut cmd = Command::new("scp");
        cmd.args(&args);
        let (status, stderr) = run_with_progress_pty(cmd, tx)?;

        if status.success() {
            let _ = tx.send(ScpEvent::Progress(100));
            return Ok(());
        }

        if crate::wallix::is_invalid_target(&stderr) {
            refused.push(endpoint);
            continue;
        }
        // La destination est affichée pour comparer avec un `scp` manuel.
        anyhow::bail!(
            "scp a échoué ({status}) : {} [cible : {endpoint}]",
            scp_stderr_summary(&stderr)
        );
    }

    anyhow::bail!(
        "Wallix : cible invalide pour toutes les autorisations essayées [{}] — \
         déclarez la bonne dans wallix.authorizations",
        refused.join(", ")
    )
}

/// Extrait `(host, port)` depuis `server.host` (qui peut contenir `"host:port"`).
#[cfg(unix)]
fn resolve_host_port(server: &ResolvedServer) -> (String, u16) {
    if let Some((h, p)) = server.host.split_once(':') {
        (h.to_string(), p.parse::<u16>().unwrap_or(server.port))
    } else {
        (server.host.clone(), server.port)
    }
}

/// Authentifie la session SSH par priorité :
/// 1. Agent SSH (`$SSH_AUTH_SOCK`)
/// 2. Clé explicite (`ssh_key`, tilde expandé)
/// 3. Clés par défaut (`~/.ssh/id_ed25519`, `~/.ssh/id_rsa`, `~/.ssh/id_ecdsa`)
#[cfg(unix)]
fn auth_session(sess: &mut ssh2::Session, username: &str, ssh_key: &str) -> Result<()> {
    use std::path::PathBuf;

    // 1) Agent SSH
    if let Ok(mut agent) = sess.agent()
        && agent.connect().is_ok()
        && agent.list_identities().is_ok()
    {
        let identities = agent.identities().unwrap_or_default();
        for identity in &identities {
            if agent.userauth(username, identity).is_ok() && sess.authenticated() {
                return Ok(());
            }
        }
    }

    // 2) Clé explicite
    if !ssh_key.is_empty() {
        let expanded = shellexpand::tilde(ssh_key).to_string();
        let key_path = PathBuf::from(&expanded);
        if sess
            .userauth_pubkey_file(username, None, &key_path, None)
            .is_ok()
            && sess.authenticated()
        {
            return Ok(());
        }
    }

    // 3) Clés par défaut
    for key in &["~/.ssh/id_ed25519", "~/.ssh/id_rsa", "~/.ssh/id_ecdsa"] {
        let expanded = shellexpand::tilde(key).to_string();
        let key_path = PathBuf::from(&expanded);
        if key_path.exists()
            && sess
                .userauth_pubkey_file(username, None, &key_path, None)
                .is_ok()
            && sess.authenticated()
        {
            return Ok(());
        }
    }

    anyhow::bail!(
        "Authentification SSH échouée pour {} (agent SSH + clés par défaut épuisés)",
        username
    )
}

/// Ouvre une session SSH vers la cible en passant par un jump host.
///
/// ## Principe
///
/// 1. Connexion TCP → jump host, session SSH2 **avec compression** (réduit les données
///    chiffrées 2× sur le réseau), authentification.
/// 2. Ouverture d'un channel `direct-tcpip` vers la cible.
/// 3. Création d'une paire de sockets Unix (`socketpair`) — pas de stack TCP locale,
///    latence quasi-nulle entre le thread bridge et la session cible.
/// 4. Thread bridge : relaie bidirectionnellement entre le socketpair et le channel.
/// 5. La session cible utilise l'extrémité locale du socketpair comme transport.
///
/// La session du jump host et le channel restent vivants dans le thread bridge
/// pour toute la durée du transfert.
#[cfg(unix)]
fn open_session_via_jump(jump_str: &str, server: &ResolvedServer) -> Result<ssh2::Session> {
    use std::net::TcpStream;
    use std::os::unix::io::FromRawFd;
    use std::os::unix::net::UnixStream;

    // ── Parse le premier saut (multi-hop : seul le premier est utilisé en v0.11) ──
    let first = jump_str.split(',').next().unwrap_or(jump_str);
    let (jump_user, jump_host_port) = match first.split_once('@') {
        Some((u, hp)) => (u, hp),
        None => (server.user.as_str(), first),
    };
    let (jump_host, jump_port) = match jump_host_port.split_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().unwrap_or(22)),
        None => (jump_host_port, 22u16),
    };

    // ── 1. Session SSH vers le jump host ──────────────────────────────────────
    let jump_tcp = TcpStream::connect(format!("{}:{}", jump_host, jump_port))?;
    // Sauvegarder le fd brut AVANT que la session en prenne ownership :
    // on l'utilisera dans le bridge pour poll() sans toucher aux données.
    let jump_raw_fd = {
        use std::os::unix::io::AsRawFd;
        jump_tcp.as_raw_fd()
    };
    let mut jump_sess = ssh2::Session::new()?;
    jump_sess.set_tcp_stream(jump_tcp);
    // Compression SSH : le CSV est très compressible ; moins de données à chiffrer
    // deux fois sur le réseau (double-encryption est le principal goulot d'étranglement).
    jump_sess.set_compress(true);
    jump_sess.handshake()?;
    auth_session(&mut jump_sess, jump_user, &server.ssh_key)?;

    // ── 2. Channel direct-tcpip vers la cible ─────────────────────────────────
    let (target_host, target_port) = resolve_host_port(server);
    let channel = jump_sess.channel_direct_tcpip(&target_host, target_port, None)?;

    // ── 3. Socketpair Unix : relie le bridge à la session cible sans TCP local ────
    let mut pair_fds: [libc::c_int; 2] = [-1; 2];
    // SOCK_CLOEXEC n'est pas défini par le crate libc sur macOS (libc ≤ 0.2.182).
    // On l'applique via fcntl après creation pour garder la portabilité.
    if unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, pair_fds.as_mut_ptr()) } != 0
    {
        anyhow::bail!("socketpair: {}", std::io::Error::last_os_error());
    }
    // Marquer les deux fds comme close-on-exec pour éviter toute fuite vers
    // d'éventuels processus enfants (portabilité Linux + macOS).
    for &fd in &pair_fds {
        unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) };
    }
    // Safety : les deux fds sont valides, on en transfère l'ownership immédiatement.
    let local_stream = unsafe { UnixStream::from_raw_fd(pair_fds[0]) };
    let bridge_stream = unsafe { UnixStream::from_raw_fd(pair_fds[1]) };

    // ── 4. Thread bridge ──────────────────────────────────────────────────────
    std::thread::spawn(move || {
        // `jump_sess` et `channel` sont déplacés ici et restent vivants
        // pour toute la durée du transfert.
        bridge_bidirectional(jump_sess, channel, bridge_stream, jump_raw_fd);
    });

    // ── 5. Session cible via le socketpair ────────────────────────────────────
    let mut target_sess = ssh2::Session::new()?;
    target_sess.set_tcp_stream(local_stream);
    target_sess.handshake()?;
    auth_session(&mut target_sess, &server.user, &server.ssh_key)?;

    Ok(target_sess)
}

/// Copie bidirectionnelle entre un channel SSH2 et un socket Unix (bridge socketpair).
///
/// Utilisé par le thread bridge dans [`open_session_via_jump`].
///
/// Utilise `libc::poll()` sur le fd TCP du jump host et l'extrémité bridge du
/// socketpair pour se réveiller uniquement quand des données sont disponibles.
/// Aucun `sleep` — latence limitée au réseau.
///
/// - `jump_raw_fd` : fd brut du socket TCP vers le jump host (sauvegardé avant
///   que `Session::set_tcp_stream` n'en prenne ownership). Utilisé uniquement
///   pour `poll()`, jamais pour lire/écrire directement.
#[cfg(unix)]
fn bridge_bidirectional(
    sess: ssh2::Session,
    mut channel: ssh2::Channel,
    mut stream: std::os::unix::net::UnixStream,
    jump_raw_fd: libc::c_int,
) {
    use std::io::{ErrorKind, Read, Write};
    use std::os::unix::io::AsRawFd;

    let stream_fd = stream.as_raw_fd();

    // Channel : non-bloquant — libssh2 retourne WouldBlock quand son buffer
    // interne est vide.
    sess.set_blocking(false);
    // stream : socket Unix bloquant (pas de Nagle, pas de stack TCP).

    let mut buf = vec![0u8; 32 * 1024]; // 32 KiB — taille max paquet SFTP

    'outer: loop {
        // Bloquer jusqu'à activité sur l'un des deux fds.
        // Timeout 50 ms pour détecter les EOF / fermetures propres sans latence perceptible.
        let mut fds = [
            libc::pollfd {
                fd: jump_raw_fd,
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: stream_fd,
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        let ret = unsafe { libc::poll(fds.as_mut_ptr(), 2, 50) };
        if ret < 0 {
            break; // EINTR ou autre erreur fatale
        }

        // ── channel → stream (remote → session SFTP locale) ──────────────────
        // On vide systématiquement le buffer interne de libssh2 même si poll
        // n'a pas signalé le jump_fd : libssh2 peut avoir des données en attente
        // déjà décryptées dans son buffer interne.
        loop {
            match channel.read(&mut buf) {
                Ok(0) => break 'outer, // EOF distant
                Ok(n) => {
                    if stream.write_all(&buf[..n]).is_err() {
                        break 'outer;
                    }
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(_) => break 'outer,
            }
        }

        // ── stream → channel (session SFTP locale → remote) ──────────────────
        if fds[1].revents & libc::POLLIN != 0 {
            match stream.read(&mut buf) {
                Ok(0) => break, // session SFTP locale fermée
                Ok(n) => {
                    // Réécriture avec retry sur WouldBlock (fenêtre SSH pleine).
                    let mut pos = 0;
                    while pos < n {
                        match channel.write(&buf[pos..n]) {
                            Ok(k) if k > 0 => pos += k,
                            Ok(_) => std::thread::yield_now(),
                            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                                std::thread::yield_now();
                            }
                            Err(_) => break 'outer,
                        }
                    }
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {} // spurious
                Err(_) => break,
            }
        }

        // EOF propre du channel (ex : serveur SFTP fermé)
        if channel.eof() {
            break;
        }
    }

    let _ = channel.close();
}

/// Tente d'expand le `~` du chemin distant via `sftp.realpath(".")`.
///
/// Si le serveur SFTP ne supporte pas `realpath`, retourne le chemin tel quel
/// (la plupart des serveurs modernes le supportent).
#[cfg(unix)]
fn resolve_remote_path(sftp: &ssh2::Sftp, remote: &str) -> std::path::PathBuf {
    use std::path::PathBuf;

    if remote.starts_with('~') {
        let home = sftp
            .realpath(std::path::Path::new("."))
            .unwrap_or_else(|_| PathBuf::from(""));
        if remote == "~" {
            home
        } else {
            // "~/foo/bar" → home.join("foo/bar")
            let tail = remote
                .trim_start_matches("~/")
                .trim_start_matches('~')
                .trim_start_matches('/');
            home.join(tail)
        }
    } else {
        PathBuf::from(remote)
    }
}

/// Lit l'acquittement SCP du serveur : `0` = OK, `1`/`2` suivi d'un message
/// terminé par `\n` = erreur (non fatale / fatale).
#[cfg(unix)]
fn read_scp_ack(r: &mut dyn std::io::Read) -> Result<()> {
    let mut code = [0u8; 1];
    r.read_exact(&mut code)
        .map_err(|e| anyhow::anyhow!("acquittement SCP manquant : {e}"))?;
    if code[0] == 0 {
        return Ok(());
    }
    let mut msg = Vec::new();
    let mut byte = [0u8; 1];
    while r.read(&mut byte)? == 1 && byte[0] != b'\n' {
        msg.push(byte[0]);
    }
    anyhow::bail!(
        "SCP refusé par le serveur : {}",
        String::from_utf8_lossy(&msg).trim()
    )
}

/// Copie `src` vers `dst` par chunks de 64 KiB en émettant [`ScpEvent::Progress`].
///
/// N'émet un événement que si le pourcentage change (évite de saturer le channel).
/// Garantit l'émission de `100%` en fin de transfert même si `total == 0`.
#[cfg(unix)]
fn copy_with_progress(
    src: &mut dyn std::io::Read,
    dst: &mut dyn std::io::Write,
    total: u64,
    tx: &mpsc::Sender<ScpEvent>,
) -> Result<()> {
    // 256 KiB : donne à libssh2 suffisamment de données pour pipeliner plusieurs
    // paquets SFTP (max 32 KiB chacun) sans attendre des ACKs intermédiaires.
    let mut buf = vec![0u8; 262144];
    let mut transferred: u64 = 0;
    let mut last_pct: u8 = 0;

    loop {
        let n = src.read(&mut buf)?;
        if n == 0 {
            break;
        }
        dst.write_all(&buf[..n])?;
        transferred += n as u64;

        let pct = (transferred * 100).checked_div(total).unwrap_or(0).min(100) as u8;

        if pct != last_pct {
            // Arrête si le receiver est droppé (transfert annulé côté TUI).
            if tx.send(ScpEvent::Progress(pct)).is_err() {
                anyhow::bail!("transfert annulé");
            }
            last_pct = pct;
        }
    }

    // Garantit 100% même si total=0 ou par arrondi.
    if last_pct < 100 {
        let _ = tx.send(ScpEvent::Progress(100));
    }

    Ok(())
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConnectionMode;

    #[test]
    #[cfg(unix)]
    fn scp_ack_zero_is_ok() {
        let mut r = std::io::Cursor::new(vec![0u8]);
        assert!(read_scp_ack(&mut r).is_ok());
    }

    #[test]
    #[cfg(unix)]
    fn scp_ack_error_reports_message() {
        let mut r = std::io::Cursor::new(b"\x01scp: /x/a.txt: Permission denied\n".to_vec());
        let err = read_scp_ack(&mut r).unwrap_err().to_string();
        assert!(err.contains("Permission denied"), "{err}");
    }

    #[test]
    #[cfg(unix)]
    fn scp_ack_fatal_reports_message() {
        let mut r = std::io::Cursor::new(b"\x02fatal\n".to_vec());
        assert!(
            read_scp_ack(&mut r)
                .unwrap_err()
                .to_string()
                .contains("fatal")
        );
    }

    #[test]
    #[cfg(unix)]
    fn scp_ack_eof_is_error() {
        let mut r = std::io::Cursor::new(Vec::new());
        assert!(read_scp_ack(&mut r).is_err());
    }

    /// Les avertissements d'OpenSSH (known_hosts, post-quantique) sont retirés
    /// pour ne garder que la cause réelle de l'échec.
    #[test]
    #[cfg(unix)]
    fn scp_stderr_summary_drops_openssh_noise() {
        let stderr = "Warning: Permanently added 'b.example' (ECDSA) to the list of known hosts.\r\n\
** WARNING: connection is not using a post-quantum key exchange algorithm.\n\
** This session may be vulnerable to \"store now, decrypt later\" attacks.\n\
** The server may need to be upgraded. See https://openssh.com/pq.html\n\
jdoe@srv+jdoe@b.example: Permission denied (publickey,keyboard-interactive).\n\
scp: Connection closed\n";
        assert_eq!(
            scp_stderr_summary(stderr),
            "jdoe@srv+jdoe@b.example: Permission denied (publickey,keyboard-interactive). | scp: Connection closed"
        );
    }

    #[test]
    #[cfg(unix)]
    fn scp_stderr_summary_keeps_raw_when_only_noise() {
        assert_eq!(scp_stderr_summary("** WARNING: x\n"), "** WARNING: x");
    }

    /// Ligne du progress meter d'OpenSSH, rafraîchie par `\r`.
    #[test]
    #[cfg(unix)]
    fn scp_progress_parses_meter_line() {
        let mut p = ScpProgressParser::default();
        assert_eq!(
            p.feed(b"\rbackup.tar.gz                  42%  420MB  12.3MB/s   00:47 ETA"),
            Some(42)
        );
    }

    /// Plusieurs rafraîchissements dans un même chunk : le dernier l'emporte.
    #[test]
    #[cfg(unix)]
    fn scp_progress_keeps_latest_update() {
        let mut p = ScpProgressParser::default();
        let chunk = b"\rf.bin   10%  1MB 1.0MB/s 00:09 ETA\rf.bin   20%  2MB 1.0MB/s 00:08 ETA";
        assert_eq!(p.feed(chunk), Some(20));
    }

    /// Ligne finale terminée par `\r\n` (onlcr du pty).
    #[test]
    #[cfg(unix)]
    fn scp_progress_parses_final_line() {
        let mut p = ScpProgressParser::default();
        assert_eq!(
            p.feed(b"\rf.bin  100%   10MB  5.0MB/s   00:02    \r\n"),
            Some(100)
        );
    }

    /// Une ligne coupée entre deux lectures n'émet rien tant que le
    /// pourcentage n'est pas suivi d'autre chose.
    #[test]
    #[cfg(unix)]
    fn scp_progress_handles_split_chunks() {
        let mut p = ScpProgressParser::default();
        assert_eq!(p.feed(b"\rf.bin   1"), None);
        assert_eq!(p.feed(b"5%"), None);
        assert_eq!(p.feed(b"  1MB 1.0MB/s 00:09 ETA"), Some(15));
    }

    /// Un nom de fichier qui ressemble à un pourcentage n'est pas pris pour
    /// la progression.
    #[test]
    #[cfg(unix)]
    fn scp_progress_ignores_percent_in_filename() {
        let mut p = ScpProgressParser::default();
        assert_eq!(p.feed(b"\r99%"), None);
        assert_eq!(p.feed(b"   7%  1MB 1.0MB/s 00:09 ETA"), Some(7));
    }

    /// Sortie sans progress meter (bannière, texte libre) : rien.
    #[test]
    #[cfg(unix)]
    fn scp_progress_ignores_unrelated_output() {
        let mut p = ScpProgressParser::default();
        assert_eq!(p.feed(b"Welcome to the bastion\r\n"), None);
        assert_eq!(p.feed(b"\rf.bin  250%  x y z"), None);
    }

    /// Bout en bout : un faux `scp` qui écrit un progress meter seulement si
    /// sa sortie est un terminal au premier plan (comme OpenSSH) est bien lu.
    #[test]
    #[cfg(unix)]
    fn run_with_progress_pty_reads_meter_from_tty_child() {
        let script = r#"[ -t 1 ] || exit 3
[ "$(ps -o tpgid= -p $$ | tr -d ' ')" = "$(ps -o pgid= -p $$ | tr -d ' ')" ] || exit 4
printf '\rf.bin   30%%  3MB 1MB/s 00:07 ETA'
sleep 0.3
printf '\rf.bin  100%%  10MB 1MB/s 00:00    \n'"#;
        let (tx, rx) = mpsc::channel();
        let mut cmd = std::process::Command::new("sh");
        cmd.args(["-c", script]);
        let (status, _stderr) = run_with_progress_pty(cmd, &tx).unwrap();
        assert!(status.success(), "{status}");
        let seen: Vec<u8> = rx
            .try_iter()
            .filter_map(|e| match e {
                ScpEvent::Progress(p) => Some(p),
                _ => None,
            })
            .collect();
        assert!(seen.contains(&30), "{seen:?}");
        assert_eq!(seen.last(), Some(&100), "{seen:?}");
    }

    /// La sortie d'erreur reste capturée séparément.
    #[test]
    #[cfg(unix)]
    fn run_with_progress_pty_captures_stderr() {
        let (tx, _rx) = mpsc::channel();
        let mut cmd = std::process::Command::new("sh");
        cmd.args(["-c", "echo 'Permission denied' >&2; exit 1"]);
        let (status, stderr) = run_with_progress_pty(cmd, &tx).unwrap();
        assert!(!status.success());
        assert!(stderr.contains("Permission denied"), "{stderr}");
    }

    /// Serveur Wallix type : groupe YAML `Colibris`, env `PR`, groupe Wallix
    /// `ces3s-admins`.
    fn wallix_server() -> ResolvedServer {
        let mut server = base_server();
        server.host = "srv01.example.fr".into();
        server.user = "jdoe".into();
        server.group_name = "Colibris".into();
        server.env_name = "PR".into();
        server.wallix_group = Some("ces3s-admins".into());
        server.bastion_host = Some("ssh.bastion.example.fr".into());
        server.bastion_user = Some("jdoe".into());
        server
    }

    fn endpoint(args: &[String], direction: &ScpDirection) -> String {
        let n = args.len();
        match direction {
            ScpDirection::Upload => args[n - 1].clone(),
            ScpDirection::Download => args[n - 2].clone(),
        }
    }

    // ── Arguments scp ─────────────────────────────────────────────────────────

    /// Format validé sur le bastion :
    /// `user@cible+SSH+autorisation+bastion_user@bastion:chemin`.
    #[test]
    #[cfg(unix)]
    fn wallix_scp_args_upload() {
        let args = build_wallix_scp_args(
            &wallix_server(),
            &ScpDirection::Upload,
            "/tmp/a.txt",
            "/data/",
            Some("PR-COLIBRIS_ces3s-admins"),
        )
        .expect("args");
        assert!(args.contains(&"-A".to_string()), "{args:?}");
        assert!(args.contains(&"BatchMode=yes".to_string()), "{args:?}");
        assert!(args.contains(&"LogLevel=ERROR".to_string()), "{args:?}");
        let n = args.len();
        assert_eq!(args[n - 2], "/tmp/a.txt");
        assert_eq!(
            args[n - 1],
            "jdoe@srv01.example.fr+SSH+PR-COLIBRIS_ces3s-admins+jdoe@ssh.bastion.example.fr:/data/"
        );
    }

    #[test]
    #[cfg(unix)]
    fn wallix_scp_args_download() {
        let args = build_wallix_scp_args(
            &wallix_server(),
            &ScpDirection::Download,
            "/tmp/",
            "~/f.log",
            Some("PR-X_y"),
        )
        .expect("args");
        let n = args.len();
        assert_eq!(
            args[n - 2],
            "jdoe@srv01.example.fr+SSH+PR-X_y+jdoe@ssh.bastion.example.fr:~/f.log"
        );
        assert_eq!(args[n - 1], "/tmp/");
    }

    #[test]
    #[cfg(unix)]
    fn wallix_scp_args_without_authorization() {
        let args = build_wallix_scp_args(&wallix_server(), &ScpDirection::Upload, "/a", "/b", None)
            .expect("args");
        assert_eq!(
            endpoint(&args, &ScpDirection::Upload),
            "jdoe@srv01.example.fr+SSH+jdoe@ssh.bastion.example.fr:/b"
        );
    }

    /// Port du bastion via `-P`, port éventuel de la cible retiré du login.
    #[test]
    #[cfg(unix)]
    fn wallix_scp_args_ports_and_key() {
        let mut server = wallix_server();
        server.host = "srv01.example.fr:2222".into();
        server.bastion_host = Some("ssh.bastion.example.fr:2200".into());
        server.ssh_key = "/k/id".into();
        let args = build_wallix_scp_args(&server, &ScpDirection::Upload, "/tmp/a", "/d/", None)
            .expect("args");
        let joined = args.join(" ");
        assert!(joined.contains("-P 2200"), "{joined}");
        assert!(joined.contains("-i /k/id"), "{joined}");
        assert!(
            endpoint(&args, &ScpDirection::Upload).starts_with("jdoe@srv01.example.fr+SSH+"),
            "{args:?}"
        );
    }

    #[test]
    #[cfg(unix)]
    fn wallix_scp_args_without_bastion_host_errors() {
        let mut server = wallix_server();
        server.bastion_host = None;
        assert!(build_wallix_scp_args(&server, &ScpDirection::Upload, "/a", "/b", None).is_err());
    }

    /// En mode Wallix, `spawn_sftp` lance le transfert ; un bastion injoignable
    /// remonte une erreur via le channel.
    #[test]
    #[cfg(unix)]
    fn wallix_unreachable_bastion_emits_error_event() {
        let mut server = base_server();
        server.bastion_host = Some("127.0.0.1:1".into());
        server.bastion_user = Some("jdoe".into());

        let rx = spawn_sftp(
            &server,
            ConnectionMode::Wallix,
            ScpDirection::Upload,
            "/tmp/file",
            "/remote/file",
        )
        .expect("spawn_sftp ne doit pas échouer immédiatement en mode Wallix");

        // Ignore FileSize / Progress : seul l'évènement terminal compte.
        let event = loop {
            match rx.recv_timeout(std::time::Duration::from_secs(10)) {
                Ok(ScpEvent::FileSize(_) | ScpEvent::Progress(_)) => continue,
                other => break other,
            }
        };
        assert!(
            matches!(event, Ok(ScpEvent::Error(_))),
            "attendu ScpEvent::Error, obtenu {:?}",
            event
        );
    }

    /// Vérifie que `spawn_sftp` retourne une erreur (via le channel) quand le
    /// serveur SSH est injoignable — sans bloquer indéfiniment.
    #[test]
    #[cfg(unix)]
    fn unreachable_server_emits_error_event() {
        let mut server = base_server();
        // Port invalide sur loopback — connexion refusée quasi-instantanément.
        server.host = "127.0.0.1".into();
        server.port = 1; // port fermé garanti

        let rx = spawn_sftp(
            &server,
            ConnectionMode::Direct,
            ScpDirection::Upload,
            "/tmp/file",
            "/remote/file",
        )
        .expect("spawn_sftp ne doit pas échouer immédiatement");

        // Le thread doit émettre un ScpEvent::Error (connexion refusée).
        let event = rx.recv_timeout(std::time::Duration::from_secs(5));
        assert!(
            matches!(event, Ok(ScpEvent::Error(_))),
            "attendu ScpEvent::Error, obtenu {:?}",
            event
        );
    }

    fn base_server() -> ResolvedServer {
        ResolvedServer {
            namespace: String::new(),
            group_name: String::new(),
            env_name: String::new(),
            name: "test".into(),
            host: "127.0.0.1".into(),
            user: "test".into(),
            port: 22,
            ssh_key: String::new(),
            ssh_options: vec![],
            default_mode: ConnectionMode::Direct,
            jump_host: None,
            bastion_host: None,
            bastion_user: None,
            bastion_template: String::new(),
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
}
