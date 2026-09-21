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
//! | Wallix   | ✖ retourne une erreur |

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
///
/// **Mode Wallix non supporté** — retourne une erreur immédiatement.
#[cfg(unix)]
pub fn spawn_sftp(
    server: &ResolvedServer,
    mode: ConnectionMode,
    direction: ScpDirection,
    local: &str,
    remote: &str,
) -> Result<mpsc::Receiver<ScpEvent>> {
    if mode == ConnectionMode::Wallix {
        anyhow::bail!("SFTP non disponible en mode Wallix");
    }

    let server = server.clone();
    let local = shellexpand::tilde(local).into_owned();
    let remote = remote.to_string();

    let (tx, rx) = mpsc::channel::<ScpEvent>();

    std::thread::spawn(move || {
        let result = transfer_inner(&server, mode, &direction, &local, &remote, &tx);
        match result {
            Ok(()) => {
                let _ = tx.send(ScpEvent::Done(true));
            }
            Err(e) => {
                let _ = tx.send(ScpEvent::Error(e.to_string()));
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
        let sftp = sess.sftp()?;
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
            let mut channel = sess.scp_send(&remote_path, 0o644, file_size, None)?;
            copy_with_progress(&mut src, &mut channel, file_size, tx)?;
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
            let (mut channel, scp_stat) = sess.scp_recv(&remote_path)?;
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
        ConnectionMode::Wallix => anyhow::bail!("SFTP non disponible en mode Wallix"),
    }
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

    /// Vérifie que `spawn_sftp` retourne immédiatement une erreur en mode Wallix
    /// sans lancer de thread ni de connexion réseau.
    #[test]
    #[cfg(unix)]
    fn wallix_returns_error_immediately() {
        let server = base_server();
        let result = spawn_sftp(
            &server,
            ConnectionMode::Wallix,
            ScpDirection::Upload,
            "/tmp/file",
            "/remote/file",
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Wallix"));
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
            wallix_header_columns: vec![],
        }
    }
}
