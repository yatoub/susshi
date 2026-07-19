use std::{collections::HashSet, io, process, time::Duration};

use clap::Parser;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseButton,
        MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend, layout::Rect};

use susshi::app::{
    App, AppMode, CmdState, ConfigItem, ScpState, TunnelOverlayState, WallixSelectorState,
    WizardState,
};
use susshi::config::{Config, ConnectionMode, IncludeWarning, ResolvedServer, undefined_vars};
use susshi::fl;
use susshi::handlers::{get_layout, handle_mouse_event, is_in_rect};
use susshi::import;
use susshi::probe::ProbeState;
use susshi::ssh::client::build_ssh_args;
use susshi::ssh::sftp::ScpDirection;
use susshi::state;
use susshi::ui;

// ─── CLI ─────────────────────────────────────────────────────────────────────

use susshi::Cli;

// ─── Config minimale de premier lancement ─────────────────────────────────────
//
// Écrite une fois quand ~/.susshi.yml n'existe pas encore, pour que
// Config::load_merged ait toujours un fichier valide à lire. Le wizard
// (touche déclenchée dans main() juste avant d'entrer en mode TUI) propose
// ensuite d'y ajouter un premier groupe/serveur ; Esc laisse ce squelette vide.

const MINIMAL_CONFIG: &str = "groups: []\n";

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Valide le fichier de configuration, affiche les diagnostics et quitte le processus.
///
/// Code de sortie :
/// - `0` : configuration valide (avec ou sans avertissements)
/// - `1` : fichier introuvable ou erreur de parsing
fn validate_config(config_path: &std::path::Path) {
    if !config_path.exists() {
        eprintln!(
            "ERREUR : fichier de configuration introuvable : {}",
            config_path.display()
        );
        process::exit(1);
    }

    let mut stack = HashSet::new();
    match Config::load_merged(config_path, &mut stack, 0) {
        Err(e) => {
            eprintln!("ERREUR : {e}");
            process::exit(1);
        }
        Ok((config, inc_warnings, val_warnings)) => {
            let mut has_error = false;

            for w in &inc_warnings {
                match w {
                    IncludeWarning::LoadError { label, path, error } => {
                        eprintln!("[ERREUR include] {label} ({path}): {error}");
                        has_error = true;
                    }
                    IncludeWarning::Circular { label, path } => {
                        eprintln!("[WARN  circular] {label} ({path})");
                    }
                }
            }
            for w in &val_warnings {
                eprintln!("[WARN  yaml]    {w}");
            }
            // Vérification des variables de template non définies
            let empty = std::collections::HashMap::new();
            let mut vars_warnings: usize = 0;
            if let Ok(resolved_servers) = config.resolve() {
                for srv in &resolved_servers {
                    let fields = [
                        ("name", srv.name.as_str()),
                        ("host", srv.host.as_str()),
                        ("user", srv.user.as_str()),
                        ("ssh_key", srv.ssh_key.as_str()),
                    ];
                    for (field_name, value) in fields {
                        for var in undefined_vars(value, &empty) {
                            eprintln!(
                                "[WARN  vars]    {} ({}/{}): champ \u{ab} {} \u{bb} contient \u{ab} {{{{ {} }}}} \u{bb} non d\u{e9}fini",
                                srv.name, srv.namespace, srv.group_name, field_name, var
                            );
                            vars_warnings += 1;
                        }
                    }
                }
            }
            let total_warnings = inc_warnings.len() + val_warnings.len() + vars_warnings;
            if has_error {
                process::exit(1);
            } else if total_warnings == 0 {
                println!("Configuration valide \u{2713}");
            } else {
                println!(
                    "Configuration valide avec {} avertissement(s)",
                    total_warnings
                );
            }
            process::exit(0);
        }
    }
}

/// Importe `~/.ssh/config` et écrit (ou affiche) le YAML susshi généré.
fn run_import_ssh_config(cli: &Cli) {
    let default_path = shellexpand::tilde("~/.ssh/config").into_owned();
    let path_str = cli.ssh_config_path.as_deref().unwrap_or(&default_path);
    let path = std::path::Path::new(path_str);

    let result = import::import_ssh_config(path);

    for w in &result.warnings {
        eprintln!("[WARN] {w}");
    }

    if result.entries.is_empty() {
        eprintln!("Aucune entrée trouvée dans {path_str}");
        process::exit(1);
    }

    let yaml = import::import_to_yaml(&result.entries);

    if cli.dry_run {
        print!("{yaml}");
        eprintln!(
            "{} entrée(s) importée(s) (dry-run, rien n\'a été écrit).",
            result.entries.len()
        );
        process::exit(0);
    }

    match &cli.output {
        Some(out_path) => {
            if let Err(e) = std::fs::write(out_path, &yaml) {
                eprintln!("Erreur écriture {out_path} : {e}");
                process::exit(1);
            }
            println!(
                "{} entrée(s) importée(s) → {out_path}",
                result.entries.len()
            );
        }
        None => {
            print!("{yaml}");
            eprintln!("{} entrée(s) importée(s).", result.entries.len());
        }
    }
    process::exit(0);
}

/// Exporte la configuration susshi vers un inventaire au format `format`.
///
fn run_export(cli: &Cli, config: &Config) {
    use susshi::export::ansible;
    use susshi::export::csv;
    use susshi::export::openssh;

    let format = cli.export.as_deref().unwrap_or("");
    if !matches!(format, "ansible" | "csv" | "openssh") {
        eprintln!("Format d'export inconnu : {format}. Formats supportés : ansible, csv, openssh");
        process::exit(1);
    }

    let servers = match config.resolve() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Erreur lors de la résolution de la configuration : {e}");
            process::exit(1);
        }
    };
    let filter = cli.export_filter.as_deref().unwrap_or("");
    let filtered = ansible::filter_servers(&servers, filter);

    if filtered.is_empty() {
        eprintln!("Aucun serveur ne correspond au filtre {:?}.", filter);
        process::exit(1);
    }

    let output = match format {
        "csv" => csv::to_csv_string(&filtered),
        "openssh" => openssh::to_openssh_config(&filtered),
        _ => ansible::to_ansible_yaml(&filtered),
    };

    match &cli.export_output {
        Some(path) => {
            if let Err(e) = std::fs::write(path, &output) {
                eprintln!("Erreur écriture {path} : {e}");
                process::exit(1);
            }
            eprintln!("{} serveur(s) exporté(s) → {path}", filtered.len());
        }
        None => {
            print!("{output}");
            eprintln!("{} serveur(s) exporté(s).", filtered.len());
        }
    }
    process::exit(0);
}

/// Liste les serveurs en JSON sur stdout.
fn run_list(cli: &Cli, config: &Config) {
    use susshi::export::ansible;

    let servers = match config.resolve() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Erreur lors de la résolution de la configuration : {e}");
            process::exit(1);
        }
    };
    let filter = cli.list_filter.as_deref().unwrap_or("");
    let filtered = ansible::filter_servers(&servers, filter);

    let json_servers: Vec<serde_json::Value> = filtered
        .iter()
        .map(|s| {
            serde_json::json!({
                "name": s.name,
                "host": s.host,
                "user": s.user,
                "port": s.port,
                "group": s.group_name,
                "env": s.env_name,
                "namespace": s.namespace,
                "tags": s.tags,
                "mode": format!("{:?}", s.default_mode).to_lowercase(),
            })
        })
        .collect();

    match serde_json::to_string_pretty(&json_servers) {
        Ok(json) => println!("{json}"),
        Err(e) => {
            eprintln!("Erreur sérialisation JSON : {e}");
            process::exit(1);
        }
    }
    process::exit(0);
}

/// Exécute une commande SSH sur tous les serveurs du groupe en parallèle.
fn run_exec_group(cli: &Cli, config: &Config) {
    let group = cli.exec_group.as_deref().unwrap_or("");
    let cmd = match cli.exec_cmd.as_deref() {
        Some(c) if !c.trim().is_empty() => c,
        _ => {
            eprintln!("--exec-cmd est requis avec --exec-group");
            process::exit(1);
        }
    };
    let timeout_secs = cli.exec_timeout;

    let servers = match config.resolve() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Erreur lors de la résolution de la configuration : {e}");
            process::exit(1);
        }
    };

    let targets: Vec<_> = servers
        .iter()
        .filter(|s| s.group_name.eq_ignore_ascii_case(group))
        .collect();

    if targets.is_empty() {
        eprintln!("Aucun serveur trouvé dans le groupe {:?}.", group);
        process::exit(1);
    }

    eprintln!(
        "Exécution de {:?} sur {} serveur(s) du groupe {:?}…",
        cmd,
        targets.len(),
        group
    );

    let cmd = cmd.to_string();
    let handles: Vec<_> = targets
        .iter()
        .map(|srv| {
            let srv = (*srv).clone();
            let cmd = cmd.clone();
            std::thread::spawn(move || {
                let mut args =
                    match susshi::ssh::client::build_ssh_args(&srv, srv.default_mode, false) {
                        Ok(a) => a,
                        Err(e) => return (srv.name.clone(), Err(e.to_string())),
                    };
                // Ajouter la commande distante avant la destination
                // L'invariant build_ssh_args garantit que la destination est en dernier.
                let dest = args.pop().unwrap_or_default();
                args.push("-o".into());
                args.push(format!("ConnectTimeout={timeout_secs}"));
                args.push("-o".into());
                args.push("BatchMode=yes".into());
                args.push(dest);
                args.push(cmd.clone());

                let output = std::process::Command::new("ssh").args(&args).output();

                match output {
                    Ok(out) => {
                        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
                        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
                        let code = out.status.code().unwrap_or(-1);
                        (srv.name.clone(), Ok((stdout, stderr, code)))
                    }
                    Err(e) => (srv.name.clone(), Err(e.to_string())),
                }
            })
        })
        .collect();

    let mut exit_code = 0i32;
    for handle in handles {
        match handle.join() {
            Ok((name, Ok((stdout, stderr, code)))) => {
                println!("=== {name} (exit {code}) ===");
                if !stdout.is_empty() {
                    print!("{stdout}");
                }
                if !stderr.is_empty() {
                    eprint!("{stderr}");
                }
                if code != 0 {
                    exit_code = 1;
                }
            }
            Ok((name, Err(e))) => {
                eprintln!("=== {name} (erreur) ===");
                eprintln!("{e}");
                exit_code = 1;
            }
            Err(_) => {
                eprintln!("Thread SSH a paniqué");
                exit_code = 1;
            }
        }
    }
    process::exit(exit_code);
}

/// Décompose `[user@]host[:port]` en ses parties.
fn parse_target(s: &str) -> (Option<String>, String, Option<u16>) {
    let (user, rest) = if let Some((u, r)) = s.split_once('@') {
        (Some(u.to_string()), r)
    } else {
        (None, s)
    };
    let (host, port) = if let Some((h, p)) = rest.split_once(':') {
        (h.to_string(), p.parse().ok())
    } else {
        (rest.to_string(), None)
    };
    (user, host, port)
}

/// Construit un `ResolvedServer` minimal pour une connexion sans TUI.
fn build_adhoc_server(
    target: &str,
    mode: ConnectionMode,
    cli: &Cli,
    config: &Config,
) -> ResolvedServer {
    let (parsed_user, host, parsed_port) = parse_target(target);
    let d = config.defaults.clone().unwrap_or_default();

    let user = cli
        .user
        .clone()
        .or(parsed_user)
        .or(d.user.clone())
        .unwrap_or_else(|| "root".to_string());
    let port = cli.port.or(parsed_port).or(d.ssh_port).unwrap_or(22);
    let ssh_key = cli
        .key
        .clone()
        .or(d.ssh_key.clone())
        .unwrap_or_else(|| "~/.ssh/id_rsa".to_string());
    let ssh_options = d.ssh_options.clone().unwrap_or_default();

    let jump_host = d.jump.as_ref().map(|jumps| {
        jumps
            .iter()
            .map(|j| {
                let h = j.host.as_deref().unwrap_or("");
                let u = j.user.as_deref().unwrap_or(&user);
                format!("{u}@{h}")
            })
            .collect::<Vec<_>>()
            .join(",")
    });
    let bastion_host = d.wallix.as_ref().and_then(|b| b.host.clone());
    let bastion_user = d.wallix.as_ref().and_then(|b| b.user.clone());
    let bastion_template = d
        .wallix
        .as_ref()
        .and_then(|b| b.template.clone())
        .unwrap_or_else(|| "{target_user}@%n:SSH:{bastion_user}".to_string());

    ResolvedServer {
        namespace: String::new(),
        group_name: String::new(),
        env_name: String::new(),
        name: host.clone(),
        host,
        user,
        port,
        ssh_key,
        ssh_cert: String::new(),
        ssh_options,
        default_mode: mode,
        jump_host,
        bastion_host,
        bastion_user,
        bastion_template,
        use_system_ssh_config: d.use_system_ssh_config.unwrap_or(false),
        probe_filesystems: vec![],
        tunnels: vec![],
        tags: vec![],
        control_master: false,
        agent_forwarding: false,
        control_path: String::new(),
        control_persist: "10m".to_string(),
        pre_connect_hook: d
            .pre_connect_hook
            .as_deref()
            .map(|h| shellexpand::tilde(h).into_owned()),
        post_disconnect_hook: d
            .post_disconnect_hook
            .as_deref()
            .map(|h| shellexpand::tilde(h).into_owned()),
        hook_timeout_secs: d.hook_timeout_secs.unwrap_or(5),
        notes: String::new(),
        ssh_agent_sock: String::new(),
        wallix_group: None,
        wallix_account: d
            .wallix
            .as_ref()
            .and_then(|b| b.account.clone())
            .unwrap_or_else(|| "default".to_string()),
        wallix_protocol: d
            .wallix
            .as_ref()
            .and_then(|b| b.protocol.clone())
            .unwrap_or_else(|| "SSH".to_string()),
        wallix_auto_select: d
            .wallix
            .as_ref()
            .and_then(|b| b.auto_select)
            .unwrap_or(true),
        wallix_fail_if_menu_match_error: d
            .wallix
            .as_ref()
            .and_then(|b| b.fail_if_menu_match_error)
            .unwrap_or(true),
        wallix_selection_timeout_secs: d
            .wallix
            .as_ref()
            .and_then(|b| b.selection_timeout_secs)
            .unwrap_or(8),
        wallix_direct: d.wallix.as_ref().and_then(|b| b.direct).unwrap_or(false),
        wallix_authorization: d.wallix.as_ref().and_then(|b| b.authorization.clone()),
        wallix_header_columns: d
            .wallix
            .as_ref()
            .and_then(|b| b.header_columns.clone())
            .unwrap_or_default(),
    }
}

// ─── main ─────────────────────────────────────────────────────────────────────

fn main() -> io::Result<()> {
    susshi::i18n::init();

    let cli = Cli::parse();

    // Résolution du chemin de config
    let config_path_str = cli
        .config
        .clone()
        .unwrap_or_else(|| shellexpand::tilde("~/.susshi.yml").into_owned());
    let config_path = std::path::Path::new(&config_path_str);

    // ── Mode validation ─────────────────────────────────────────────────────
    if cli.validate {
        validate_config(config_path);
        // validate_config appelle process::exit() — on ne revient jamais ici.
    }
    // ── Mode import ssh_config ───────────────────────────────────────────────────
    if cli.import_ssh_config {
        run_import_ssh_config(&cli);
        // run_import_ssh_config appelle process::exit()
    }
    let is_first_run = !config_path.exists();
    if is_first_run && let Err(e) = std::fs::write(config_path, MINIMAL_CONFIG) {
        eprintln!("Failed to create default config: {}", e);
        return Err(e);
    }

    let (config, warnings, val_warnings) =
        match Config::load_merged(config_path, &mut HashSet::new(), 0) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to load config: {}", e);
                return Err(io::Error::new(io::ErrorKind::InvalidData, e.to_string()));
            }
        };

    // ── Modes non-TUI ──────────────────────────────────────────────────────
    if cli.export.is_some() {
        run_export(&cli, &config);
        // run_export appelle process::exit()
    }
    if cli.list {
        run_list(&cli, &config);
        // run_list appelle process::exit()
    }
    if cli.exec_group.is_some() {
        run_exec_group(&cli, &config);
        // run_exec_group appelle process::exit()
    }

    let cli_mode_target: Option<(ConnectionMode, String)> = cli
        .direct
        .as_deref()
        .map(|t| (ConnectionMode::Direct, t.to_string()))
        .or_else(|| {
            cli.jump
                .as_deref()
                .map(|t| (ConnectionMode::Jump, t.to_string()))
        })
        .or_else(|| {
            cli.wallix
                .as_deref()
                .map(|t| (ConnectionMode::Wallix, t.to_string()))
        });

    if let Some((mode, target)) = cli_mode_target {
        let server = build_adhoc_server(&target, mode, &cli, &config);
        if let Err(e) = susshi::hooks::run_hook(
            server.pre_connect_hook.as_deref().unwrap_or(""),
            "pre_connect",
            &server,
        ) {
            eprintln!("Hook pre_connect a annulé la connexion : {e}");
            return Err(io::Error::other(e.to_string()));
        }
        // post_disconnect_hook non supporté ici : exec() remplace le processus.
        if let Err(e) = susshi::ssh::client::connect(&server, mode, cli.verbose, None) {
            eprintln!("SSH Connection Error: {}", e);
            return Err(io::Error::other(e.to_string()));
        }
        return Ok(()); // exec() remplace le process ; on n'arrive jamais ici
    }

    // ── Mode TUI normal ─────────────────────────────────────────────────────
    let mut app = App::new(config, warnings, config_path.to_path_buf(), val_warnings)
        .map_err(io::Error::other)?;

    if is_first_run {
        app.start_wizard();
    }

    // Délai courant de backoff pour la reconnexion automatique (mode keep_open).
    // Initialisé à 0, puis remis à 0 à chaque nouvelle connexion volontaire.
    #[allow(unused_assignments)]
    let mut backoff_secs: u32 = 0;

    loop {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let res = run_app(&mut terminal, &mut app);

        // Persiste l'état avant de quitter la TUI
        state::save_state(&app.to_app_state());

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        match res {
            Ok(AppResult::Exit) => break,
            Ok(AppResult::Connect(server, mode, verbose)) => {
                backoff_secs = 0;
                if app.keep_open {
                    // Connexion bloquante avec reconnexion automatique et backoff.
                    if let Err(e) = susshi::hooks::run_hook(
                        server.pre_connect_hook.as_deref().unwrap_or(""),
                        "pre_connect",
                        &server,
                    ) {
                        eprintln!("Hook pre_connect a annulé la connexion : {e}");
                    } else {
                        run_blocking_with_backoff(
                            || susshi::ssh::client::connect_blocking(&server, mode, verbose, None),
                            &mut backoff_secs,
                        );
                        let _ = susshi::hooks::run_hook(
                            server.post_disconnect_hook.as_deref().unwrap_or(""),
                            "post_disconnect",
                            &server,
                        );
                    }
                    // Boucle → ré-ouvre la TUI
                } else {
                    // Comportement historique : exec() remplace le processus.
                    if let Err(e) = susshi::hooks::run_hook(
                        server.pre_connect_hook.as_deref().unwrap_or(""),
                        "pre_connect",
                        &server,
                    ) {
                        eprintln!("Hook pre_connect a annulé la connexion : {e}");
                    } else {
                        // post_disconnect_hook non supporté ici : exec() remplace le processus.
                        if let Err(e) = susshi::ssh::client::connect(&server, mode, verbose, None) {
                            eprintln!("SSH Connection Error: {}", e);
                        }
                    }
                    break;
                }
            }
            Ok(AppResult::ConnectWallixSelected(server, verbose, selected_id, auth)) => {
                backoff_secs = 0;
                if app.keep_open {
                    if let Err(e) = susshi::hooks::run_hook(
                        server.pre_connect_hook.as_deref().unwrap_or(""),
                        "pre_connect",
                        &server,
                    ) {
                        eprintln!("Hook pre_connect a annulé la connexion : {e}");
                    } else {
                        run_blocking_with_backoff(
                            || {
                                susshi::ssh::client::connect_blocking_wallix_with_selection(
                                    &server,
                                    verbose,
                                    &selected_id,
                                    auth.as_deref(),
                                )
                            },
                            &mut backoff_secs,
                        );
                        let _ = susshi::hooks::run_hook(
                            server.post_disconnect_hook.as_deref().unwrap_or(""),
                            "post_disconnect",
                            &server,
                        );
                    }
                } else {
                    if let Err(e) = susshi::hooks::run_hook(
                        server.pre_connect_hook.as_deref().unwrap_or(""),
                        "pre_connect",
                        &server,
                    ) {
                        eprintln!("Hook pre_connect a annulé la connexion : {e}");
                    } else if let Err(e) = susshi::ssh::client::connect_wallix_with_selection(
                        &server,
                        verbose,
                        &selected_id,
                        auth.as_deref(),
                    ) {
                        eprintln!("SSH Connection Error: {}", e);
                    }
                    break;
                }
            }
            Ok(AppResult::ConnectWithAuth(server, mode, verbose, cred)) => {
                backoff_secs = 0;
                if app.keep_open {
                    if let Err(e) = susshi::hooks::run_hook(
                        server.pre_connect_hook.as_deref().unwrap_or(""),
                        "pre_connect",
                        &server,
                    ) {
                        eprintln!("Hook pre_connect a annulé la connexion : {e}");
                    } else {
                        run_blocking_with_backoff(
                            || {
                                susshi::ssh::client::connect_blocking(
                                    &server,
                                    mode,
                                    verbose,
                                    Some(cred.as_str()),
                                )
                            },
                            &mut backoff_secs,
                        );
                        let _ = susshi::hooks::run_hook(
                            server.post_disconnect_hook.as_deref().unwrap_or(""),
                            "post_disconnect",
                            &server,
                        );
                    }
                } else {
                    if let Err(e) = susshi::hooks::run_hook(
                        server.pre_connect_hook.as_deref().unwrap_or(""),
                        "pre_connect",
                        &server,
                    ) {
                        eprintln!("Hook pre_connect a annulé la connexion : {e}");
                    } else if let Err(e) =
                        susshi::ssh::client::connect(&server, mode, verbose, Some(cred.as_str()))
                    {
                        eprintln!("SSH Connection Error: {}", e);
                    }
                    break;
                }
            }
            Err(err) => {
                eprintln!("Application Error: {:?}", err);
                break;
            }
        }
    }

    Ok(())
}

// ─── Reconnexion avec backoff ─────────────────────────────────────────────────

/// Exécute `connect_fn`, puis si elle échoue et que `keep_open` est actif,
/// attend un délai croissant (1 → 2 → 4 → 8 → 16 → 30 s, cap 30 s)
/// avant de signaler à l'appelant de relancer la TUI.
///
/// Si la connexion a duré plus de `SESSION_STABLE_SECS` secondes, le backoff
/// est remis à zéro (déconnexion réseau normale, pas un échec immédiat).
fn run_blocking_with_backoff<F>(connect_fn: F, backoff_secs: &mut u32)
where
    F: FnOnce() -> anyhow::Result<()>,
{
    const SESSION_STABLE_SECS: u64 = 10;
    const BACKOFF_CAP: u32 = 30;

    let started = std::time::Instant::now();
    match connect_fn() {
        Ok(()) => {
            // Connexion terminée normalement.
            if started.elapsed().as_secs() >= SESSION_STABLE_SECS {
                *backoff_secs = 0;
            }
        }
        Err(e) => {
            eprintln!("SSH Connection Error: {e}");
            if started.elapsed().as_secs() >= SESSION_STABLE_SECS {
                *backoff_secs = 0;
            } else {
                let delay = (*backoff_secs).max(1);
                eprintln!("Reconnexion dans {delay}s… (Ctrl-C pour annuler)");
                std::thread::sleep(std::time::Duration::from_secs(u64::from(delay)));
                *backoff_secs = (delay * 2).min(BACKOFF_CAP);
            }
        }
    }
}

// ─── TUI ─────────────────────────────────────────────────────────────────────

pub enum AppResult {
    Exit,
    Connect(Box<susshi::config::ResolvedServer>, ConnectionMode, bool),
    ConnectWallixSelected(
        Box<susshi::config::ResolvedServer>,
        bool,
        String,
        Option<String>,
    ),
    /// Connexion avec un credential (passphrase ou mot de passe) saisi dans la TUI.
    ConnectWithAuth(
        Box<susshi::config::ResolvedServer>,
        ConnectionMode,
        bool,
        String,
    ),
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> io::Result<AppResult> {
    let mut last_click_time = std::time::Instant::now();
    let mut last_click_pos = (0, 0);

    loop {
        let size_obj = terminal.size()?;
        let size = Rect::new(0, 0, size_obj.width, size_obj.height);

        terminal.draw(|f| ui::draw(f, app))?;

        // Expire le message de statut après 3 secondes
        if let Some((_, ts)) = &app.status_message
            && ts.elapsed() > Duration::from_secs(3)
        {
            app.status_message = None;
        }

        // Lit le résultat du diagnostic si un thread tourne
        if let Some(rx) = &app.probe_rx
            && let Ok(result) = rx.try_recv()
        {
            app.probe_state = match result {
                Ok(probe) => ProbeState::Done(probe),
                Err(msg) => ProbeState::Error(msg),
            };
            app.probe_rx = None;
        }

        // Lit le résultat du diagnostic du serveur épinglé
        if let Some(rx) = &app.pinned_probe_rx
            && let Ok(result) = rx.try_recv()
        {
            app.pinned_probe_state = match result {
                Ok(probe) => ProbeState::Done(probe),
                Err(msg) => ProbeState::Error(msg),
            };
            app.pinned_probe_rx = None;
        }

        // Lit le résultat de la commande ad-hoc si un thread tourne
        app.poll_cmd();

        // Lit le résultat du chargement du menu Wallix si un thread tourne
        app.poll_wallix_selector();

        if let Some((server, selected_id, auth)) = app.take_pending_wallix_connection() {
            app.record_connection(&server);
            return Ok(AppResult::ConnectWallixSelected(
                Box::new(server),
                app.verbose_mode,
                selected_id,
                auth,
            ));
        }

        // Sonde l'état des tunnels SSH actifs (détecte les fins inopinées)
        app.poll_tunnel_events();

        // Sonde les évènements du transfert SCP en cours
        app.poll_scp_events();

        // Sonde les résultats des probes parallèles du dashboard overview
        app.poll_overview();

        if event::poll(Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(key) => {
                    if !matches!(app.wizard_state, WizardState::Idle) {
                        match key.code {
                            KeyCode::Esc => app.wizard_cancel(),
                            KeyCode::Enter => {
                                if let Err(e) = app.wizard_submit() {
                                    app.app_mode = AppMode::Error(e.to_string());
                                }
                            }
                            KeyCode::Tab => app.wizard_next_field(),
                            KeyCode::BackTab => app.wizard_prev_field(),
                            KeyCode::Char(c) => app.wizard_push_char(c),
                            KeyCode::Backspace => app.wizard_backspace(),
                            _ => {}
                        }
                    } else if let AppMode::CredentialInput { .. } = &app.app_mode {
                        match key.code {
                            KeyCode::Esc => {
                                app.cancel_credential_input();
                            }
                            KeyCode::Enter => {
                                if let Some((server, mode, verbose, cred)) =
                                    app.submit_credential_input()
                                {
                                    app.record_connection(&server);
                                    return Ok(AppResult::ConnectWithAuth(
                                        Box::new(server),
                                        mode,
                                        verbose,
                                        cred,
                                    ));
                                }
                            }
                            KeyCode::Char(c) => {
                                app.credential_input_push(c);
                            }
                            KeyCode::Backspace => {
                                app.credential_input_backspace();
                            }
                            _ => {}
                        }
                    } else if matches!(app.app_mode, AppMode::ClipboardFallback(_)) {
                        match key.code {
                            KeyCode::Esc | KeyCode::Enter => app.app_mode = AppMode::Normal,
                            _ => {}
                        }
                    } else if app.app_mode != AppMode::Normal {
                        // En mode erreur : n'importe quelle touche ferme le panneau
                        match key.code {
                            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => app.clear_error(),
                            _ => {}
                        }
                    } else if matches!(app.cmd_state, CmdState::Prompting(_)) {
                        // Mode saisie commande ad-hoc
                        match key.code {
                            KeyCode::Esc => {
                                app.reset_cmd();
                            }
                            KeyCode::Enter => {
                                if let CmdState::Prompting(buf) = app.cmd_state.clone() {
                                    if !buf.trim().is_empty() {
                                        if let Some(server) = app.selected_server() {
                                            app.start_cmd(&server, buf.trim().to_string());
                                        }
                                    } else {
                                        app.reset_cmd();
                                    }
                                }
                            }
                            KeyCode::Up if !app.cmd_history.is_empty() => {
                                let cursor = match app.cmd_history_cursor {
                                    None => app.cmd_history.len() - 1,
                                    Some(c) => c.saturating_sub(1),
                                };
                                app.cmd_history_cursor = Some(cursor);
                                let entry = app.cmd_history[cursor].clone();
                                app.cmd_state = CmdState::Prompting(entry);
                            }
                            KeyCode::Down => {
                                if let Some(cursor) = app.cmd_history_cursor {
                                    if cursor + 1 < app.cmd_history.len() {
                                        let next = cursor + 1;
                                        app.cmd_history_cursor = Some(next);
                                        let entry = app.cmd_history[next].clone();
                                        app.cmd_state = CmdState::Prompting(entry);
                                    } else {
                                        app.cmd_history_cursor = None;
                                        app.cmd_state = CmdState::Prompting(String::new());
                                    }
                                }
                            }
                            KeyCode::Char(c) => {
                                app.cmd_history_cursor = None;
                                if let CmdState::Prompting(ref mut buf) = app.cmd_state {
                                    buf.push(c);
                                }
                            }
                            KeyCode::Backspace => {
                                app.cmd_history_cursor = None;
                                if let CmdState::Prompting(ref mut buf) = app.cmd_state {
                                    buf.pop();
                                }
                            }
                            _ => {}
                        }
                    } else if matches!(app.scp_state, ScpState::SelectingDirection) {
                        // Sélection de la direction SCP
                        match key.code {
                            KeyCode::Up | KeyCode::Char('u') | KeyCode::Char('U') => {
                                app.scp_select_direction(ScpDirection::Upload);
                            }
                            KeyCode::Down | KeyCode::Char('d') | KeyCode::Char('D') => {
                                app.scp_select_direction(ScpDirection::Download);
                            }
                            KeyCode::Esc | KeyCode::Char('q') => {
                                app.close_scp_overlay();
                            }
                            _ => {}
                        }
                    } else if matches!(app.scp_state, ScpState::FillingForm { .. }) {
                        // Formulaire SCP
                        match key.code {
                            KeyCode::Char(c) => app.scp_form_char(c),
                            KeyCode::Backspace => app.scp_form_backspace(),
                            KeyCode::Tab | KeyCode::BackTab => app.scp_form_next_field(),
                            KeyCode::Enter => app.scp_form_submit(),
                            KeyCode::Esc => app.close_scp_overlay(),
                            _ => {}
                        }
                    } else if matches!(app.scp_state, ScpState::Done { .. } | ScpState::Error(_)) {
                        // Résultat SCP — n'importe quelle touche ferme
                        match key.code {
                            KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q') => {
                                app.dismiss_scp_result();
                            }
                            _ => {}
                        }
                    } else if matches!(
                        &app.wallix_selector,
                        Some(WallixSelectorState::Loading { .. })
                    ) {
                        if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
                            app.close_wallix_selector();
                        }
                    } else if matches!(
                        &app.wallix_selector,
                        Some(WallixSelectorState::Error { .. })
                    ) {
                        if matches!(key.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q')) {
                            app.close_wallix_selector();
                        }
                    } else if matches!(&app.wallix_selector, Some(WallixSelectorState::List { .. }))
                    {
                        match key.code {
                            KeyCode::Down | KeyCode::Char('j') => app.wallix_selector_next(),
                            KeyCode::Up | KeyCode::Char('k') => app.wallix_selector_previous(),
                            KeyCode::Esc | KeyCode::Char('q') => app.close_wallix_selector(),
                            KeyCode::Enter => {
                                if let Some((server, selected_id)) =
                                    app.wallix_selector_selected_id()
                                {
                                    app.remember_wallix_selection(&server, &selected_id);
                                    app.close_wallix_selector();
                                    app.record_connection(&server);
                                    return Ok(AppResult::ConnectWallixSelected(
                                        Box::new(server),
                                        app.verbose_mode,
                                        selected_id,
                                        app.wallix_pending_auth.take(),
                                    ));
                                }
                            }
                            _ => {}
                        }
                    } else if matches!(&app.tunnel_overlay, Some(TunnelOverlayState::Form(_))) {
                        // Mode formulaire d'édition / création de tunnel
                        match key.code {
                            KeyCode::Char(c) => app.tunnel_form_char(c),
                            KeyCode::Backspace => app.tunnel_form_backspace(),
                            KeyCode::Tab => app.tunnel_form_next_field(),
                            KeyCode::BackTab => app.tunnel_form_prev_field(),
                            KeyCode::Enter => app.tunnel_form_submit(),
                            KeyCode::Esc => app.tunnel_form_cancel(),
                            _ => {}
                        }
                    } else if matches!(&app.tunnel_overlay, Some(TunnelOverlayState::List { .. })) {
                        // Mode liste de tunnels
                        match key.code {
                            KeyCode::Down | KeyCode::Char('j') => app.tunnel_overlay_next(),
                            KeyCode::Up | KeyCode::Char('k') => app.tunnel_overlay_previous(),
                            KeyCode::Enter => app.tunnel_overlay_toggle(),
                            KeyCode::Delete => app.tunnel_overlay_delete(),
                            KeyCode::Char('e') => app.open_tunnel_form_edit(),
                            KeyCode::Char('a') => app.open_tunnel_form_add(),
                            KeyCode::Char('q') | KeyCode::Esc => app.close_tunnel_overlay(),
                            _ => {}
                        }
                    } else if app.is_searching {
                        match key.code {
                            KeyCode::Enter | KeyCode::Esc => {
                                app.is_searching = false;
                            }
                            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                app.search_query.clear();
                                app.selected_index = 0;
                                app.list_state.select(Some(0));
                                app.invalidate_cache();
                            }
                            KeyCode::Char(c) => {
                                app.search_query.push(c);
                                app.selected_index = 0;
                                app.list_state.select(Some(0));
                                app.invalidate_cache();
                            }
                            KeyCode::Backspace => {
                                app.search_query.pop();
                                app.selected_index = 0;
                                app.list_state.select(Some(0));
                                app.invalidate_cache();
                            }
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('q') => {
                                return Ok(AppResult::Exit);
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if app.overview.is_some() {
                                    app.overview_scroll_down();
                                } else {
                                    app.next();
                                }
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                if app.overview.is_some() {
                                    app.overview_scroll_up();
                                } else {
                                    app.previous();
                                }
                            }
                            KeyCode::Tab => {
                                app.connection_mode = app.connection_mode.next();
                            }
                            KeyCode::Char('1') => {
                                app.connection_mode = ConnectionMode::Direct;
                            }
                            KeyCode::Char('2') => {
                                app.connection_mode = ConnectionMode::Jump;
                            }
                            KeyCode::Char('3') => {
                                app.connection_mode = ConnectionMode::Wallix;
                            }
                            KeyCode::Char('v') => {
                                app.verbose_mode = !app.verbose_mode;
                            }
                            KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                use susshi::config::ThemeVariant;
                                use susshi::ui::theme::get_theme;
                                app.theme_variant = match app.theme_variant {
                                    ThemeVariant::Latte => ThemeVariant::Frappe,
                                    ThemeVariant::Frappe => ThemeVariant::Macchiato,
                                    ThemeVariant::Macchiato => ThemeVariant::Mocha,
                                    ThemeVariant::Mocha => ThemeVariant::Latte,
                                };
                                app.theme = get_theme(app.theme_variant);
                            }
                            KeyCode::Char('y') => {
                                let items = app.get_visible_items();
                                if let Some(ConfigItem::Server(server)) =
                                    items.get(app.selected_index)
                                {
                                    match build_ssh_args(
                                        server,
                                        app.connection_mode,
                                        app.verbose_mode,
                                    ) {
                                        Ok(args) => {
                                            let cmd = format!("ssh {}", args.join(" "));
                                            match app.clipboard.as_mut().map(|cb| cb.set_text(&cmd))
                                            {
                                                Some(Ok(_)) => app.set_status_message(fl!(
                                                    "copied",
                                                    cmd = cmd.as_str()
                                                )),
                                                Some(Err(_)) | None => {
                                                    app.app_mode = AppMode::ClipboardFallback(cmd);
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            let err = e.to_string();
                                            app.set_status_message(fl!(
                                                "ssh-error",
                                                error = err.as_str()
                                            ));
                                        }
                                    }
                                }
                            }
                            KeyCode::Char('/') => {
                                app.is_searching = true;
                            }
                            KeyCode::Char('h') => {
                                app.show_help = !app.show_help;
                            }
                            KeyCode::Char('M') => {
                                app.mouse_capture = !app.mouse_capture;
                                if app.mouse_capture {
                                    execute!(terminal.backend_mut(), EnableMouseCapture)?;
                                } else {
                                    execute!(terminal.backend_mut(), DisableMouseCapture)?;
                                }
                            }
                            KeyCode::Esc if app.show_help => {
                                app.show_help = false;
                            }
                            KeyCode::Char('o') => {
                                if app.overview.is_some() {
                                    app.close_overview();
                                } else {
                                    app.open_overview();
                                }
                            }
                            KeyCode::Char('|') => {
                                let items = app.get_visible_items();
                                match items.get(app.selected_index) {
                                    Some(ConfigItem::Server(server)) => {
                                        if app
                                            .pinned_server
                                            .as_deref()
                                            .map(|s| s.name == server.name)
                                            .unwrap_or(false)
                                        {
                                            app.pinned_server = None;
                                            app.pinned_probe_state = ProbeState::Idle;
                                            app.pinned_probe_rx = None;
                                        } else {
                                            app.pinned_server = Some(server.clone());
                                            app.pinned_probe_state = ProbeState::Idle;
                                            app.pinned_probe_rx = None;
                                        }
                                    }
                                    _ => {
                                        app.pinned_server = None;
                                        app.pinned_probe_state = ProbeState::Idle;
                                        app.pinned_probe_rx = None;
                                    }
                                }
                            }
                            KeyCode::Esc if app.overview.is_some() => {
                                app.close_overview();
                            }
                            KeyCode::Char('r') => match app.reload() {
                                Ok(()) => {}
                                Err(_) => app.set_status_message(fl!("config-reload-error")),
                            },
                            KeyCode::Char('f') => {
                                app.toggle_favorite();
                            }
                            KeyCode::Char('F') => {
                                app.toggle_favorites_view();
                            }
                            KeyCode::Char('C') => {
                                app.collapse_all();
                            }
                            KeyCode::Char('E') => {
                                app.expand_all();
                            }
                            KeyCode::Char('H') => {
                                app.sort_by_recent = !app.sort_by_recent;
                                app.items_dirty = true;
                                let msg = if app.sort_by_recent {
                                    fl!("sort-recent-on")
                                } else {
                                    fl!("sort-recent-off")
                                };
                                app.set_status_message(msg);
                            }
                            KeyCode::Char('p') => {
                                // Prompt de credential SSH (passphrase clé ou mot de passe)
                                let items = app.get_visible_items();
                                if let Some(ConfigItem::Server(server)) =
                                    items.get(app.selected_index)
                                {
                                    let has_key = !server.ssh_key.is_empty();
                                    app.open_credential_input(has_key);
                                }
                            }
                            KeyCode::Char('x') => {
                                // Lance la saisie de commande ad-hoc
                                let items = app.get_visible_items();
                                if matches!(
                                    items.get(app.selected_index),
                                    Some(ConfigItem::Server(_))
                                ) {
                                    app.cmd_state = CmdState::Prompting(String::new());
                                }
                            }
                            KeyCode::Char('T') => {
                                // Ouvre l'overlay des tunnels SSH pour le serveur sélectionné
                                let items = app.get_visible_items();
                                if matches!(
                                    items.get(app.selected_index),
                                    Some(ConfigItem::Server(_))
                                ) {
                                    app.open_tunnel_overlay();
                                }
                            }
                            KeyCode::Char('s') => {
                                // Ouvre le transfert SCP pour le serveur sélectionné
                                let items = app.get_visible_items();
                                if matches!(
                                    items.get(app.selected_index),
                                    Some(ConfigItem::Server(_))
                                ) {
                                    app.open_scp_select_direction();
                                }
                            }
                            KeyCode::Esc
                                if matches!(
                                    app.cmd_state,
                                    CmdState::Done { .. } | CmdState::Error(_)
                                ) =>
                            {
                                app.reset_cmd();
                            }
                            KeyCode::Char('d') => {
                                let items = app.get_visible_items();
                                if let Some(ConfigItem::Server(server)) =
                                    items.get(app.selected_index)
                                {
                                    let server_clone = (**server).clone();
                                    let mode = app.connection_mode;
                                    let (tx, rx) = std::sync::mpsc::channel();
                                    app.probe_rx = Some(rx);
                                    app.probe_state = ProbeState::Running;
                                    std::thread::spawn(move || {
                                        let result = susshi::probe::probe(&server_clone, mode)
                                            .map_err(|e| e.to_string());
                                        let _ = tx.send(result);
                                    });
                                }
                                if let Some(pinned) = &app.pinned_server {
                                    let server_clone = (**pinned).clone();
                                    let mode = app.connection_mode;
                                    let (tx, rx) = std::sync::mpsc::channel();
                                    app.pinned_probe_rx = Some(rx);
                                    app.pinned_probe_state = ProbeState::Running;
                                    std::thread::spawn(move || {
                                        let result = susshi::probe::probe(&server_clone, mode)
                                            .map_err(|e| e.to_string());
                                        let _ = tx.send(result);
                                    });
                                }
                            }
                            KeyCode::Char(' ') => {
                                app.toggle_expansion();
                            }
                            KeyCode::Enter => {
                                let action = {
                                    let items = app.get_visible_items();
                                    match items.get(app.selected_index) {
                                        Some(ConfigItem::Server(server)) => {
                                            match build_ssh_args(
                                                server,
                                                app.connection_mode,
                                                app.verbose_mode,
                                            ) {
                                                Ok(_) => Some(Ok(Box::new((**server).clone()))),
                                                Err(e) => Some(Err(format!("{e}"))),
                                            }
                                        }
                                        _ => None,
                                    }
                                };
                                match action {
                                    Some(Ok(server)) => {
                                        if app.should_open_wallix_selector(&server) {
                                            app.open_wallix_selector(
                                                (*server).clone(),
                                                app.verbose_mode,
                                            );
                                        } else {
                                            app.record_connection(&server);
                                            return Ok(AppResult::Connect(
                                                server,
                                                app.connection_mode,
                                                app.verbose_mode,
                                            ));
                                        }
                                    }
                                    Some(Err(msg)) => app.set_error(msg),
                                    None => app.toggle_expansion(),
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Event::Mouse(mouse) => {
                    if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                        let handled = handle_mouse_event(mouse, app, size)?;

                        let now = std::time::Instant::now();
                        if handled
                            && now.duration_since(last_click_time) < Duration::from_millis(400)
                            && last_click_pos == (mouse.column, mouse.row)
                        {
                            let layout = get_layout(size);
                            if is_in_rect(mouse.column, mouse.row, layout.list_area) {
                                let action = {
                                    let items = app.get_visible_items();
                                    match items.get(app.selected_index) {
                                        Some(ConfigItem::Server(server)) => {
                                            match build_ssh_args(
                                                server,
                                                app.connection_mode,
                                                app.verbose_mode,
                                            ) {
                                                Ok(_) => Some(Ok(Box::new((**server).clone()))),
                                                Err(e) => Some(Err(format!("{e}"))),
                                            }
                                        }
                                        _ => None,
                                    }
                                };
                                match action {
                                    Some(Ok(server)) => {
                                        if app.should_open_wallix_selector(&server) {
                                            app.open_wallix_selector(
                                                (*server).clone(),
                                                app.verbose_mode,
                                            );
                                        } else {
                                            app.record_connection(&server);
                                            return Ok(AppResult::Connect(
                                                server,
                                                app.connection_mode,
                                                app.verbose_mode,
                                            ));
                                        }
                                    }
                                    Some(Err(msg)) => app.set_error(msg),
                                    None => {}
                                }
                            }
                        }
                        last_click_time = now;
                        last_click_pos = (mouse.column, mouse.row);
                    }
                }
                _ => {}
            }
        }
    }
}
