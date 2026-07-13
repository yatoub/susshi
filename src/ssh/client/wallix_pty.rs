use super::*;

/// Crée un script shell temporaire qui affiche `credential` sur stdout, utilisé
/// comme `SSH_ASKPASS`. Le script est créé avec les permissions 700.
/// L'appelant est responsable de supprimer le fichier après usage.
#[cfg(unix)]
pub(crate) fn setup_askpass_script(credential: &str) -> Result<std::path::PathBuf> {
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt as _;
    let path = std::env::temp_dir().join(format!("susshi-askpass-{}", std::process::id()));
    let escaped = credential.replace('\'', r"'\''");
    let script = format!("#!/bin/sh\nprintf '%s\\n' '{}'\n", escaped);
    let mut f = std::fs::File::create(&path)?;
    f.write_all(script.as_bytes())?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
    Ok(path)
}

#[cfg(unix)]
pub(crate) fn wallix_connected_directly(buffer: &str) -> bool {
    let clean = crate::wallix::strip_ansi(buffer);
    let lower = clean.to_ascii_lowercase();
    // Wallix prints this after bypassing the menu and opening a direct shell.
    lower.contains("account successfully checked out")
        && !lower.contains("tapez h pour")
        && !lower.contains("type h for")
}

#[cfg(unix)]
pub(crate) fn current_winsize() -> Option<Winsize> {
    let mut winsize = Winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };

    let rc = unsafe { libc::ioctl(libc::STDIN_FILENO, libc::TIOCGWINSZ, &mut winsize) };
    if rc == 0 { Some(winsize) } else { None }
}

/// Détecte une demande d'authentification SSH (passphrase de clé ou mot de passe).
/// Utilisé pour intercepter ces prompts dans la boucle PTY Wallix.
#[cfg(unix)]
pub(crate) fn contains_ssh_auth_prompt(buffer: &str) -> bool {
    let lower = buffer.to_ascii_lowercase();
    lower.contains("enter passphrase for key") || lower.contains("password:")
}

#[cfg(unix)]
pub(crate) fn contains_wallix_prompt(buffer: &str) -> bool {
    let clean = crate::wallix::strip_ansi(buffer);
    let trimmed = clean.trim_end();
    trimmed.ends_with(" >")
        || trimmed.ends_with(">")
        || trimmed.lines().rev().find(|line| !line.trim().is_empty()) == Some(">")
}

#[cfg(unix)]
pub(crate) fn contains_wallix_target_address_prompt(buffer: &str) -> bool {
    let clean = crate::wallix::strip_ansi(buffer);
    let lowered = clean.to_ascii_lowercase();
    lowered.contains("adresse cible")
        || lowered.contains("target address")
        || lowered.contains("destination address")
}

#[cfg(unix)]
pub(crate) fn contains_wallix_return_selector_prompt(buffer: &str) -> bool {
    let clean = crate::wallix::strip_ansi(buffer);
    let lowered = clean.to_lowercase();
    lowered.contains("retour au sélecteur")
        || lowered.contains("retour au selecteur")
        || lowered.contains("return to selector")
}

#[cfg(unix)]
pub(crate) fn parse_wallix_page_position(buffer: &str) -> Option<(u32, u32)> {
    let clean = crate::wallix::strip_ansi(buffer);
    let lowered = clean.to_ascii_lowercase();
    let marker = "page ";
    let start = lowered.rfind(marker)? + marker.len();
    let tail = &lowered[start..];

    let mut current = String::new();
    let mut total = String::new();
    let mut seen_slash = false;

    for character in tail.chars() {
        if character.is_ascii_digit() {
            if seen_slash {
                total.push(character);
            } else {
                current.push(character);
            }
        } else if character == '/' && !seen_slash {
            seen_slash = true;
        } else if !current.is_empty() {
            break;
        }
    }

    if current.is_empty() || total.is_empty() {
        return None;
    }

    Some((current.parse().ok()?, total.parse().ok()?))
}

#[cfg(unix)]
pub(crate) fn is_wallix_menu_matching_error(err: &anyhow::Error) -> bool {
    let message = err.to_string();
    message.contains("No menu entry found with target")
        || message.contains("No menu entry found for matching targets")
        || message.contains("No menu entry found for target")
}

#[cfg(unix)]
pub(crate) fn spawn_wallix_pty(
    args: &[String],
) -> Result<(nix::unistd::Pid, std::fs::File, std::fs::File)> {
    let mut argv = Vec::with_capacity(args.len() + 2);
    argv.push(CString::new("ssh")?);
    for arg in args {
        argv.push(CString::new(arg.as_str())?);
    }
    let mut argv_ptrs: Vec<*const libc::c_char> = argv.iter().map(|arg| arg.as_ptr()).collect();
    argv_ptrs.push(std::ptr::null());

    let winsize = current_winsize();
    let fork = unsafe { forkpty(winsize.as_ref(), None) }
        .map_err(|err| anyhow::anyhow!("Failed to create PTY for Wallix session: {err}"))?;

    match fork {
        ForkptyResult::Child => unsafe {
            libc::execvp(argv[0].as_ptr(), argv_ptrs.as_ptr());
            libc::_exit(127);
        },
        ForkptyResult::Parent { child, master } => {
            let master_reader = std::fs::File::from(master);
            let master_writer = master_reader.try_clone()?;
            Ok((child, master_reader, master_writer))
        }
    }
}

#[cfg(unix)]
pub(crate) fn connect_wallix_via_pty_with_selection(
    server: &ResolvedServer,
    verbose: bool,
    selected_id: Option<&str>,
    auth: Option<&str>,
) -> Result<()> {
    let args = build_wallix_bastion_args(server, verbose)?;
    let (child, mut master_reader, mut master_writer) = spawn_wallix_pty(&args)?;
    let mut stdout = std::io::stdout().lock();
    let mut stdin = std::io::stdin().lock();
    let mut transcript = String::new();
    let mut selection_completed = false;
    let mut target_address_sent = false;
    let mut return_selector_prompt_handled = false;
    let mut stdin_closed = false;
    // Si un ID est déjà connu (fallback TUI), on masque le menu global Wallix
    // pour éviter l'affichage interactif dans le terminal utilisateur.
    let hide_menu_output = selected_id.is_some();
    // `auth_prompted` devient true dès qu'un prompt SSH auth est détecté.
    // Quand true, on affiche toujours la sortie et on active stdin pour que
    // l'utilisateur puisse répondre (ou pour injecter le credential automatiquement).
    let mut auth_prompted = false;
    let master_fd = master_reader.as_raw_fd();
    let stdin_fd = std::io::stdin().as_raw_fd();

    loop {
        let mut pollfds = [
            libc::pollfd {
                fd: master_fd,
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: if stdin_closed || !(selection_completed || auth_prompted && auth.is_none()) {
                    -1
                } else {
                    stdin_fd
                },
                events: libc::POLLIN,
                revents: 0,
            },
        ];

        let rc = unsafe { libc::poll(pollfds.as_mut_ptr(), pollfds.len() as _, 100) };
        if rc < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(err.into());
        }

        if pollfds[0].revents & libc::POLLIN != 0 {
            let mut buf = [0_u8; 4096];
            let read = master_reader.read(&mut buf)?;
            if read == 0 {
                break;
            }

            let chunk = String::from_utf8_lossy(&buf[..read]);
            transcript.push_str(&chunk);
            if transcript.len() > 64 * 1024 {
                let drain = transcript.len().saturating_sub(64 * 1024);
                transcript.drain(..drain);
            }

            // Détection d'un prompt d'auth SSH avant le menu Wallix.
            if !auth_prompted && !selection_completed && contains_ssh_auth_prompt(&transcript) {
                auth_prompted = true;
                if let Some(cred) = auth {
                    // Credential connu → injection automatique silencieuse.
                    master_writer.write_all(cred.as_bytes())?;
                    master_writer.write_all(b"\n")?;
                    master_writer.flush()?;
                }
                // Si auth.is_none(), stdin est activé dans pollfds (voir ci-dessus)
                // et l'output est affiché ci-dessous pour que l'utilisateur voit le prompt.
            }

            // En mode auto-sélection, on n'affiche rien tant que la sélection n'est pas
            // envoyée afin d'éviter le bruit du menu global. Dès que selection_completed,
            // on affiche tout — le password prompt peut arriver sans "Adresse cible" selon
            // la config Wallix.
            // Exception : toujours montrer les prompts d'auth sans credential connu.
            if !hide_menu_output || selection_completed || (auth_prompted && auth.is_none()) {
                stdout.write_all(&buf[..read])?;
                stdout.flush()?;
            }

            if !selection_completed {
                if contains_wallix_prompt(&transcript) {
                    let selection = if let Some(id) = selected_id {
                        Ok(id.to_string())
                    } else {
                        parse_wallix_menu(&transcript, &server.wallix_header_columns)
                            .and_then(|entries| select_id_for_server(&entries, server))
                    };

                    match selection {
                        Ok(id) => {
                            master_writer.write_all(id.as_bytes())?;
                            master_writer.write_all(b"\n")?;
                            master_writer.flush()?;
                            selection_completed = true;
                        }
                        Err(err) if server.wallix_fail_if_menu_match_error => {
                            if is_wallix_menu_matching_error(&err) {
                                if let Some((current, total)) =
                                    parse_wallix_page_position(&transcript)
                                    && current < total
                                {
                                    master_writer.write_all(b"n\n")?;
                                    master_writer.flush()?;
                                    transcript.clear();
                                    continue;
                                }

                                // Fallback manuel: l'utilisateur choisit lui-même dans le menu.
                                selection_completed = true;
                                continue;
                            }

                            unsafe {
                                libc::kill(child.as_raw(), libc::SIGTERM);
                            }
                            let _ = waitpid(child, Some(WaitPidFlag::WNOHANG));
                            return Err(err);
                        }
                        Err(_) => {
                            selection_completed = true;
                        }
                    }
                }
            } else if !target_address_sent && contains_wallix_target_address_prompt(&transcript) {
                master_writer.write_all(server.host.as_bytes())?;
                master_writer.write_all(b"\n")?;
                master_writer.flush()?;
                target_address_sent = true;
            }

            if selection_completed
                && !return_selector_prompt_handled
                && contains_wallix_return_selector_prompt(&transcript)
            {
                // En sortie de session, Wallix peut proposer un retour au sélecteur.
                // On force un refus explicite pour terminer proprement la connexion.
                master_writer.write_all(b"n\n")?;
                master_writer.flush()?;
                return_selector_prompt_handled = true;
            }
        }

        if pollfds[1].revents & libc::POLLIN != 0 {
            let mut buf = [0_u8; 4096];
            let read = stdin.read(&mut buf)?;
            if read == 0 {
                // En mode canonique, Ctrl+D peut se traduire par EOF local (read=0).
                // On relaie explicitement un EOT vers la session distante pour
                // reproduire le comportement attendu d'un shell interactif.
                master_writer.write_all(&[0x04])?;
                master_writer.flush()?;
                stdin_closed = true;
            } else {
                master_writer.write_all(&buf[..read])?;
                master_writer.flush()?;
            }
        }

        match waitpid(child, Some(WaitPidFlag::WNOHANG)) {
            Ok(WaitStatus::StillAlive) => {}
            Ok(_) => return Ok(()),
            Err(err) => {
                return Err(anyhow::anyhow!("Failed to wait for Wallix session: {err}"));
            }
        }
    }

    if !selection_completed {
        return Err(anyhow::anyhow!(
            "Wallix session exited before menu auto-selection completed"
        ));
    }

    Ok(())
}
