//! Exécution non-interactive d'une commande distante (commande ad-hoc, probe).
//!
//! | Mode     | Transport                                                   |
//! |----------|-------------------------------------------------------------|
//! | Direct   | `ssh … user@host <commande>`                                |
//! | Jump     | idem avec `-J`                                              |
//! | Wallix   | login direct + session shell `-T`, commande envoyée sur stdin |
//!
//! En mode Wallix, la commande n'est pas passée en argument (requête `exec`,
//! sous-protocole « commande distante » souvent non autorisé) mais écrite sur
//! l'entrée d'une session shell sans PTY — le sous-protocole utilisé par les
//! connexions interactives et les tunnels. Le bastion ne fermant pas toujours
//! la session, la fin de commande est détectée par un marqueur (voir
//! [`END_MARKER`]). Chaque autorisation candidate est essayée tant que le
//! bastion répond « Cible invalide ».

use crate::config::{ConnectionMode, ResolvedServer};
use crate::ssh::client::{build_ssh_args, build_wallix_direct_args};
use anyhow::Result;
use std::io::{Read, Write as _};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Délai maximal d'une commande ad-hoc (`x`).
pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(120);

/// Sortie d'une commande distante.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecOutput {
    pub stdout: String,
    pub stderr: String,
    /// Code de sortie nul côté `ssh`.
    pub success: bool,
}

/// Options communes aux exécutions non-interactives : aucun prompt possible,
/// échec rapide si l'hôte est injoignable, sans les avertissements d'OpenSSH
/// (known_hosts, post-quantique) qui noient la sortie.
fn push_batch_options(args: &mut Vec<String>) {
    for opt in ["BatchMode=yes", "ConnectTimeout=10", "LogLevel=ERROR"] {
        args.push("-o".into());
        args.push(opt.into());
    }
}

/// Arguments `ssh` pour exécuter `command` en mode Direct ou Jump :
/// la commande est le dernier argument, juste après la destination.
pub fn build_exec_args(
    server: &ResolvedServer,
    mode: ConnectionMode,
    command: &str,
) -> Result<Vec<String>> {
    let mut args = build_ssh_args(server, mode, false)?;
    // Invariant build_ssh_args : la destination est le dernier argument.
    let destination = args
        .pop()
        .ok_or_else(|| anyhow::anyhow!("liste d'args SSH vide"))?;
    args.push("-n".into()); // stdin depuis /dev/null
    push_batch_options(&mut args);
    args.push(destination);
    args.push(command.to_string());
    Ok(args)
}

/// Arguments `ssh` d'une session shell Wallix sans PTY (`-T`) pour une
/// autorisation donnée. La commande sera envoyée sur stdin.
pub fn build_wallix_exec_args(
    server: &ResolvedServer,
    authorization: Option<&str>,
) -> Result<Vec<String>> {
    let mut args = build_wallix_direct_args(server, authorization)?;
    let destination = args
        .pop()
        .ok_or_else(|| anyhow::anyhow!("liste d'args SSH vide"))?;
    args.push("-T".into());
    push_batch_options(&mut args);
    args.push(destination);
    Ok(args)
}

/// Exécute `command` sur le serveur selon le mode de connexion.
///
/// Un code de sortie non nul n'est pas une erreur (voir [`ExecOutput::success`]) ;
/// le sont : l'échec de lancement de `ssh`, le dépassement de `timeout` et le
/// refus de toutes les autorisations Wallix candidates.
pub fn run_remote(
    server: &ResolvedServer,
    mode: ConnectionMode,
    command: &str,
    timeout: Duration,
) -> Result<ExecOutput> {
    if mode != ConnectionMode::Wallix {
        let args = build_exec_args(server, mode, command)?;
        return run_process("ssh", &args, None, timeout);
    }

    let mut refused: Vec<String> = Vec::new();
    for authorization in crate::wallix::fallback_authorizations(server) {
        let args = build_wallix_exec_args(server, authorization.as_deref())?;
        let output = run_process("ssh", &args, Some(command), timeout)?;
        if !output.success && crate::wallix::is_invalid_target(&output.stderr) {
            refused.push(crate::wallix::build_direct_login(
                server,
                authorization.as_deref(),
            ));
            continue;
        }
        return Ok(output);
    }

    anyhow::bail!(
        "Wallix : cible invalide pour toutes les autorisations essayées [{}] — \
         déclarez la bonne dans wallix.authorizations",
        refused.join(", ")
    )
}

/// Marqueur de fin émis après la commande, suivi de son code de sortie.
///
/// Via Wallix, la session ne se ferme pas forcément quand le shell distant se
/// termine (EOF non transmis, canal laissé ouvert) : le marqueur signale la
/// fin de la commande sans attendre celle du processus `ssh`.
pub(crate) const END_MARKER: &str = "__SUSSHI_END__";

/// Entrée envoyée au shell distant : la commande, le marqueur portant son
/// code de sortie, puis `exit`.
fn shell_input(command: &str) -> String {
    format!("{command}\necho \"{END_MARKER}$?\"\nexit\n")
}

/// Sépare la sortie de la commande et son code de sortie, une fois la ligne
/// du marqueur reçue en entier. `None` tant que la commande n'est pas finie.
fn split_end_marker(stdout: &str) -> Option<(String, i32)> {
    let pos = stdout.rfind(END_MARKER)?;
    let rest = &stdout[pos + END_MARKER.len()..];
    let (code, _) = rest.split_once('\n')?;
    let code = code.trim().parse().ok()?;
    Some((stdout[..pos].to_string(), code))
}

/// Lit un flux dans un tampon partagé, au fil de l'eau.
fn spawn_reader<R: Read + Send + 'static>(
    mut source: R,
) -> (Arc<Mutex<Vec<u8>>>, std::thread::JoinHandle<()>) {
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&buffer);
    let handle = std::thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        while let Ok(n) = source.read(&mut chunk) {
            if n == 0 {
                break;
            }
            if let Ok(mut buf) = sink.lock() {
                buf.extend_from_slice(&chunk[..n]);
            }
        }
    });
    (buffer, handle)
}

/// Laisse les lecteurs vider les tubes, au plus [`READER_GRACE`], sans
/// `join` bloquant : un descendant du processus (maître `ControlPersist`
/// d'OpenSSH, commande lancée en arrière-plan) peut garder stdout/stderr
/// ouverts bien après sa fin. Le thread lecteur se termine alors seul.
fn wait_readers(readers: &[&std::thread::JoinHandle<()>]) {
    let deadline = Instant::now() + READER_GRACE;
    while Instant::now() < deadline && !readers.iter().all(|r| r.is_finished()) {
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Délai accordé aux lecteurs pour vider les tubes après la fin du processus.
const READER_GRACE: Duration = Duration::from_millis(300);

fn snapshot(buffer: &Arc<Mutex<Vec<u8>>>) -> String {
    buffer
        .lock()
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default()
}

/// Lance `program` et attend la fin de la commande, au plus `timeout`.
///
/// - Sans `command` (Direct/Jump) : stdin vide, fin = sortie du processus.
/// - Avec `command` (Wallix) : la commande est écrite sur stdin (voir
///   [`shell_input`]) ; fin = marqueur reçu (le processus est alors tué) ou
///   sortie du processus.
///
/// À l'expiration du délai, le processus est tué et une erreur reprend la
/// sortie partielle.
fn run_process(
    program: &str,
    args: &[String],
    command: Option<&str>,
    timeout: Duration,
) -> Result<ExecOutput> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(if command.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("lancement de {program} : {e}"))?;

    if let (Some(command), Some(mut stdin)) = (command, child.stdin.take()) {
        // Échec d'écriture = session déjà fermée par le bastion : la cause
        // est dans stderr, lu ci-dessous.
        let _ = stdin.write_all(shell_input(command).as_bytes());
    }

    let stdout_pipe = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("stdout de {program} indisponible"))?;
    let stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("stderr de {program} indisponible"))?;
    let (stdout_buf, stdout_reader) = spawn_reader(stdout_pipe);
    let (stderr_buf, stderr_reader) = spawn_reader(stderr_pipe);

    let deadline = Instant::now() + timeout;
    let mut marker: Option<(String, i32)> = None;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if command.is_some() {
            marker = split_end_marker(&snapshot(&stdout_buf));
            if marker.is_some() {
                // Commande terminée : inutile d'attendre la fermeture de la session.
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            wait_readers(&[&stdout_reader, &stderr_reader]);
            let partial = format!("{}{}", snapshot(&stdout_buf), snapshot(&stderr_buf));
            anyhow::bail!(
                "délai dépassé ({}s) sans fin de commande : {}",
                timeout.as_secs(),
                partial.trim()
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    };

    wait_readers(&[&stdout_reader, &stderr_reader]);
    let stdout = snapshot(&stdout_buf);
    let stderr = snapshot(&stderr_buf);

    // Le processus a pu se terminer juste après avoir émis le marqueur.
    if marker.is_none() && command.is_some() {
        marker = split_end_marker(&stdout);
    }
    Ok(match marker {
        Some((output, code)) => ExecOutput {
            stdout: output,
            stderr,
            success: code == 0,
        },
        None => ExecOutput {
            stdout,
            stderr,
            success: status.is_some_and(|s| s.success()),
        },
    })
}

#[cfg(test)]
#[path = "exec_tests.rs"]
mod tests;
