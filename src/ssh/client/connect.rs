use super::*;

/// Vérifie si un socket ControlMaster SSH est actif pour ce serveur.
///
/// Retourne `true` si `ssh -O check` réussit (exit 0), `false` sinon.
/// Non bloquant : délai max ~1 s (timeout SSH interne).
pub fn is_control_master_active(server: &ResolvedServer) -> bool {
    if !server.control_master || server.control_path.is_empty() {
        return false;
    }
    let path = shellexpand::tilde(&server.control_path).into_owned();
    // Remplace les tokens SSH dans le chemin (%h, %p, %r) pour trouver le bon socket.
    let path = path
        .replace("%h", &server.host)
        .replace("%p", &server.port.to_string())
        .replace("%r", &server.user);
    if !std::path::Path::new(&path).exists() {
        return false;
    }
    Command::new("ssh")
        .args(["-O", "check", "-S", &path, &server.host])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Lance la connexion SSH en remplaçant le processus courant (`exec`).
///
/// Si `credential` est fourni, le processus courant est remplacé par un sous-processus
/// bloquant (pas d'`exec`) afin de pouvoir configurer `SSH_ASKPASS` et nettoyer le script
/// temporaire après la session.
pub fn connect(
    server: &ResolvedServer,
    mode: ConnectionMode,
    verbose: bool,
    credential: Option<&str>,
) -> Result<()> {
    if let Some(cred) = credential {
        return connect_blocking(server, mode, verbose, Some(cred));
    }
    let args = build_ssh_args(server, mode, verbose)?;
    let mut command = Command::new("ssh");
    command.args(&args);
    #[cfg(unix)]
    {
        let err = command.exec();
        Err(anyhow::Error::new(err).context("Failed to exec ssh command"))
    }
    #[cfg(not(unix))]
    {
        command
            .status()
            .map(|_| ())
            .map_err(|e| anyhow::Error::new(e).context("Failed to spawn ssh command"))
    }
}

/// Lance la connexion SSH dans un sous-processus bloquant (sans `exec`).
/// Contrairement à [`connect`], retourne après la fin de la session SSH —
/// utilisé quand `keep_open` est actif pour revenir à la TUI ensuite.
///
/// Si `credential` est fourni, `SSH_ASKPASS` est configuré pour l'injecter
/// automatiquement lorsque SSH demande une passphrase ou un mot de passe.
pub fn connect_blocking(
    server: &ResolvedServer,
    mode: ConnectionMode,
    verbose: bool,
    credential: Option<&str>,
) -> Result<()> {
    let args = build_ssh_args(server, mode, verbose)?;
    let mut command = Command::new("ssh");
    command.args(&args);

    #[cfg(unix)]
    if !server.ssh_agent_sock.is_empty() {
        command.env("SSH_AUTH_SOCK", &server.ssh_agent_sock);
    }

    #[cfg(unix)]
    let askpass_path = if let Some(cred) = credential {
        let p = setup_askpass_script(cred)?;
        command.env("SSH_ASKPASS", &p);
        command.env("SSH_ASKPASS_REQUIRE", "force");
        Some(p)
    } else {
        None
    };

    let result = command
        .status()
        .map(|_| ())
        .map_err(|e| anyhow::Error::new(e).context("Failed to spawn ssh command"));

    #[cfg(unix)]
    if let Some(p) = askpass_path {
        let _ = std::fs::remove_file(p);
    }

    result
}

/// Récupère les entrées du menu Wallix affichées par le bastion sans ouvrir de shell distant.
///
/// `auth` est un credential optionnel (passphrase de clé SSH ou mot de passe) à injecter
/// automatiquement si SSH le demande avant d'afficher le menu.
/// Si `auth` est `None` et qu'un prompt d'authentification est détecté, retourne une erreur
/// avec le préfixe `"SSH_AUTH_REQUIRED: "` pour que la TUI affiche le dialog de saisie.
#[cfg(unix)]
pub fn fetch_wallix_menu_entries(
    server: &ResolvedServer,
    verbose: bool,
    auth: Option<&str>,
) -> Result<Vec<WallixMenuEntry>> {
    let args = build_wallix_bastion_args(server, verbose)?;
    let (child, mut master_reader, mut master_writer) = spawn_wallix_pty(&args)?;
    let master_fd = master_reader.as_raw_fd();
    let mut transcript = String::new();
    let mut auth_injected = false;
    let timeout_secs = server.wallix_selection_timeout_secs;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);

    'outer: loop {
        // Poll with 200ms slices so we can check the deadline regularly.
        loop {
            if std::time::Instant::now() >= deadline {
                unsafe { libc::kill(child.as_raw(), libc::SIGTERM) };
                let _ = waitpid(child, Some(WaitPidFlag::WNOHANG));
                return Err(anyhow::anyhow!(
                    "Wallix menu fetch timed out after {}s without showing a selection prompt",
                    timeout_secs
                ));
            }

            let mut pfd = libc::pollfd {
                fd: master_fd,
                events: libc::POLLIN,
                revents: 0,
            };
            let rc = unsafe { libc::poll(&mut pfd, 1, 200) };
            if rc < 0 {
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                unsafe { libc::kill(child.as_raw(), libc::SIGTERM) };
                let _ = waitpid(child, Some(WaitPidFlag::WNOHANG));
                return Err(err.into());
            }
            if rc == 0 {
                // timeout slice — check deadline on next iteration
                continue;
            }

            // Data available.
            let mut buf = [0_u8; 4096];
            let read = match master_reader.read(&mut buf) {
                Ok(0) => break 'outer, // PTY closed
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(e) => {
                    unsafe { libc::kill(child.as_raw(), libc::SIGTERM) };
                    let _ = waitpid(child, Some(WaitPidFlag::WNOHANG));
                    return Err(e.into());
                }
            };

            let chunk = String::from_utf8_lossy(&buf[..read]);
            transcript.push_str(&chunk);
            if transcript.len() > 128 * 1024 {
                let drain = transcript.len().saturating_sub(128 * 1024);
                transcript.drain(..drain);
            }

            // Auth prompt before the menu.
            if !auth_injected && contains_ssh_auth_prompt(&transcript) {
                if let Some(cred) = auth {
                    master_writer.write_all(cred.as_bytes())?;
                    master_writer.write_all(b"\n")?;
                    master_writer.flush()?;
                    auth_injected = true;
                    transcript.clear();
                } else {
                    unsafe { libc::kill(child.as_raw(), libc::SIGTERM) };
                    let _ = waitpid(child, Some(WaitPidFlag::WNOHANG));
                    return Err(anyhow::anyhow!("SSH_AUTH_REQUIRED: {}", transcript.trim()));
                }
                continue;
            }

            if wallix_connected_directly(&transcript) {
                // The filtered login caused Wallix to connect directly without showing a menu.
                // Kill the probe PTY (the real shell is already open and then closed).
                unsafe { libc::kill(child.as_raw(), libc::SIGTERM) };
                let _ = waitpid(child, Some(WaitPidFlag::WNOHANG));
                return Err(anyhow::anyhow!("WALLIX_DIRECT_CONNECTION"));
            }

            if contains_wallix_prompt(&transcript) {
                // Check if there are more pages to fetch.
                match parse_wallix_page_position(&transcript) {
                    Some((current, total)) if current < total => {
                        master_writer.write_all(b"n\n")?;
                        master_writer.flush()?;
                        break; // go back to outer loop for next page
                    }
                    _ => break 'outer,
                }
            }
        }
    }

    unsafe {
        libc::kill(child.as_raw(), libc::SIGTERM);
    }
    let _ = waitpid(child, Some(WaitPidFlag::WNOHANG));

    if std::env::var("SUSSHI_WALLIX_DEBUG").is_ok() {
        let _ = std::fs::write("/tmp/susshi-wallix-debug.txt", &transcript);
    }

    parse_wallix_menu(&transcript, &server.wallix_header_columns)
}

#[cfg(not(unix))]
pub fn fetch_wallix_menu_entries(
    _server: &ResolvedServer,
    _verbose: bool,
    _auth: Option<&str>,
) -> Result<Vec<WallixMenuEntry>> {
    anyhow::bail!("Wallix menu fetching is only supported on Unix")
}

/// Lance une session Wallix en forçant un ID déjà choisi côté TUI.
///
/// `auth` est un credential optionnel (passphrase ou mot de passe) à injecter
/// automatiquement si SSH le demande pendant la session.
pub fn connect_wallix_with_selection(
    server: &ResolvedServer,
    verbose: bool,
    selected_id: &str,
    auth: Option<&str>,
) -> Result<()> {
    // WALLIX_DIRECT: the filtered login (bastion_user@host:protocol:bastion_user) already
    // connects without a menu. Reuse build_wallix_bastion_args so the login is identical
    // to the probe that succeeded — build_wallix_login_user uses a different format.
    if selected_id == "WALLIX_DIRECT" {
        #[cfg(unix)]
        {
            let args = build_wallix_bastion_args(server, verbose)?;
            let mut command = Command::new("ssh");
            command.args(&args);
            if let Some(cred) = auth {
                let p = setup_askpass_script(cred)?;
                command.env("SSH_ASKPASS", &p);
                command.env("SSH_ASKPASS_REQUIRE", "force");
                let result = command.status().map(|_| ()).map_err(anyhow::Error::from);
                let _ = std::fs::remove_file(p);
                return result;
            }
            let err = std::os::unix::process::CommandExt::exec(&mut command);
            return Err(anyhow::Error::new(err).context("Failed to exec ssh command"));
        }
        #[cfg(not(unix))]
        return connect(server, ConnectionMode::Wallix, verbose, auth);
    }
    #[cfg(unix)]
    {
        connect_wallix_via_pty_with_selection(server, verbose, Some(selected_id), auth)
    }
    #[cfg(not(unix))]
    {
        let _ = (server, verbose, selected_id, auth);
        anyhow::bail!("Wallix menu automation is only supported on Unix")
    }
}

/// Variante bloquante de [`connect_wallix_with_selection`].
pub fn connect_blocking_wallix_with_selection(
    server: &ResolvedServer,
    verbose: bool,
    selected_id: &str,
    auth: Option<&str>,
) -> Result<()> {
    connect_wallix_with_selection(server, verbose, selected_id, auth)
}
