use super::*;

fn build_wallix_login_user(
    server: &ResolvedServer,
    bastion_user: &str,
    target_host: &str,
) -> String {
    if server.bastion_template.trim().is_empty()
        || server.bastion_template == "{target_user}@%n:SSH:{bastion_user}"
    {
        let mut login = format!("{}@{}:{}", server.user, target_host, server.wallix_protocol);
        // wallix_authorization (exact name) takes priority over wallix_group (short name).
        let qualifier = server
            .wallix_authorization
            .as_deref()
            .map(str::trim)
            .filter(|a| !a.is_empty())
            .or_else(|| {
                server
                    .wallix_group
                    .as_deref()
                    .map(str::trim)
                    .filter(|g| !g.is_empty())
            });
        if let Some(q) = qualifier {
            login.push(':');
            login.push_str(q);
        }
        login.push(':');
        login.push_str(bastion_user);
        return login;
    }

    server
        .bastion_template
        .replace("{target_user}", &server.user)
        .replace("{target_host}", target_host)
        .replace("{bastion_user}", bastion_user)
        .replace(
            "{wallix_group}",
            server.wallix_group.as_deref().unwrap_or(""),
        )
        .replace("{protocol}", &server.wallix_protocol)
        .replace("%n", target_host)
}

/// Construit la liste complète des arguments SSH sans lancer de processus.
/// Séparé de `connect()` pour être testable unitairement.
///
/// **Invariant** : la destination (`user@host` ou `bastion_host`) est toujours
/// le **dernier** argument de la liste retournée. `probe()` s'appuie sur cet
/// invariant pour insérer ses options juste avant elle via `args.pop()`.
pub fn build_ssh_args(
    server: &ResolvedServer,
    mode: ConnectionMode,
    verbose: bool,
) -> Result<Vec<String>> {
    let mut args: Vec<String> = Vec::new();

    if !server.use_system_ssh_config {
        args.push("-F".into());
        args.push("/dev/null".into());
    }

    if verbose {
        args.push("-v".into());
    }

    // Clé et options SSH — placées AVANT la destination pour que celle-ci
    // reste en dernière position (invariant utilisé par probe()).
    if !server.ssh_key.is_empty() {
        let expanded = shellexpand::tilde(&server.ssh_key);
        args.push("-i".into());
        args.push(expanded.into_owned());
    }
    if !server.ssh_cert.is_empty() {
        let expanded = shellexpand::tilde(&server.ssh_cert);
        args.push("-i".into());
        args.push(expanded.into_owned());
    }

    for opt in &server.ssh_options {
        if opt.starts_with('-') {
            args.push(opt.clone());
        } else {
            args.push("-o".into());
            args.push(opt.clone());
        }
    }

    // Quand on ignore le ssh_config système (-F /dev/null) et que l'utilisateur
    // n'a pas explicitement configuré StrictHostKeyChecking, on injecte
    // accept-new : les nouveaux hôtes sont acceptés silencieusement, mais une
    // clé modifiée reste bloquante (comportement safe par défaut).
    if !server.use_system_ssh_config
        && !server
            .ssh_options
            .iter()
            .any(|o| o.to_ascii_lowercase().contains("stricthostkeychecking"))
    {
        args.push("-o".into());
        args.push("StrictHostKeyChecking=accept-new".into());
    }

    if server.agent_forwarding {
        args.push("-A".into());
    }

    if !server.ssh_agent_sock.is_empty() {
        let expanded = shellexpand::tilde(&server.ssh_agent_sock);
        args.push("-o".into());
        args.push(format!("IdentityAgent={}", expanded.into_owned()));
    }

    // ControlMaster SSH multiplexing (non supporté en mode Wallix).
    if server.control_master && mode != ConnectionMode::Wallix && !server.control_path.is_empty() {
        if let Some(parent) = std::path::Path::new(&server.control_path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        args.push("-o".into());
        args.push("ControlMaster=auto".into());
        args.push("-o".into());
        args.push(format!("ControlPath={}", server.control_path));
        args.push("-o".into());
        args.push(format!("ControlPersist={}", server.control_persist));
    }

    // Destination — toujours en dernier.
    match mode {
        ConnectionMode::Direct => {
            collect_target_args(&mut args, &server.user, &server.host, server.port);
        }
        ConnectionMode::Jump => {
            let jump_str = server.jump_host.as_deref().unwrap_or("");
            if jump_str.is_empty() {
                return Err(anyhow::anyhow!("Jump host not configured for this server"));
            }
            args.push("-J".into());
            args.push(jump_str.to_string());
            collect_target_args(&mut args, &server.user, &server.host, server.port);
        }
        ConnectionMode::Wallix => {
            let bastion_host_str = server.bastion_host.as_deref().unwrap_or("");
            if bastion_host_str.is_empty() {
                return Err(anyhow::anyhow!(
                    "Wallix host not configured for this server"
                ));
            }
            let bastion_user = server.bastion_user.as_deref().unwrap_or("root");
            let (t_host, _t_port) = parse_host_port(&server.host);
            let user_string = build_wallix_login_user(server, bastion_user, t_host);
            args.push("-l".into());
            args.push(user_string);
            let (b_host, b_port) = parse_host_port(bastion_host_str);
            if let Some(p) = b_port {
                args.push("-p".into());
                args.push(p.to_string());
            }
            args.push(b_host.to_string());
        }
    }

    Ok(args)
}

#[cfg(unix)]
pub(crate) fn build_wallix_bastion_args(
    server: &ResolvedServer,
    verbose: bool,
) -> Result<Vec<String>> {
    let mut args: Vec<String> = Vec::new();

    if !server.use_system_ssh_config {
        args.push("-F".into());
        args.push("/dev/null".into());
    }

    if verbose {
        args.push("-v".into());
    }

    if !server.ssh_key.is_empty() {
        let expanded = shellexpand::tilde(&server.ssh_key);
        args.push("-i".into());
        args.push(expanded.into_owned());
    }

    for opt in &server.ssh_options {
        if opt.starts_with('-') {
            args.push(opt.clone());
        } else {
            args.push("-o".into());
            args.push(opt.clone());
        }
    }

    if !server.use_system_ssh_config
        && !server
            .ssh_options
            .iter()
            .any(|o| o.to_ascii_lowercase().contains("stricthostkeychecking"))
    {
        args.push("-o".into());
        args.push("StrictHostKeyChecking=accept-new".into());
    }

    let bastion_host_str = server.bastion_host.as_deref().unwrap_or("");
    if bastion_host_str.is_empty() {
        return Err(anyhow::anyhow!(
            "Wallix host not configured for this server"
        ));
    }

    let bastion_user = server.bastion_user.as_deref().unwrap_or("root");
    args.push("-l".into());
    // Pass target host in the login to let Wallix filter the menu server-side.
    // When wallix_authorization is set (e.g. "STI-ANSCORE_ces3s-admins"), include it so
    // Wallix connects directly without showing a selection menu.
    // Without authorization, target-only filtering reduces a 27-page menu to 1-2 entries.
    let (t_host, _) = parse_host_port(&server.host);
    let filtered_login = if let Some(auth) = server
        .wallix_authorization
        .as_deref()
        .filter(|a| !a.is_empty())
    {
        format!(
            "{}@{}:{}:{}:{}",
            bastion_user, t_host, server.wallix_protocol, auth, bastion_user
        )
    } else {
        format!(
            "{}@{}:{}:{}",
            bastion_user, t_host, server.wallix_protocol, bastion_user
        )
    };
    args.push(filtered_login);

    let (b_host, b_port) = parse_host_port(bastion_host_str);
    if let Some(p) = b_port {
        args.push("-p".into());
        args.push(p.to_string());
    }
    args.push(b_host.to_string());

    Ok(args)
}

fn collect_target_args(args: &mut Vec<String>, user: &str, host_str: &str, server_port: u16) {
    let (host, embedded_port) = parse_host_port(host_str);
    // Priorité : port embarqué dans host_str (ex. "host:2222") puis server.port.
    let port = embedded_port
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(server_port);
    if port != 22 {
        args.push("-p".into());
        args.push(port.to_string());
    }
    args.push(format!("{}@{}", user, host));
}

fn parse_host_port(s: &str) -> (&str, Option<&str>) {
    if let Some((host, port)) = s.split_once(':') {
        (host, Some(port))
    } else {
        (s, None)
    }
}
