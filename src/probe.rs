//! Diagnostic rapide de connexion SSH.
//!
//! La fonction [`probe`] exécute un one-liner bash sur le serveur distant et
//! retourne les métriques système dans un [`ProbeResult`].

use crate::config::{ConnectionMode, ResolvedServer};
use crate::ssh::exec::run_remote;
use anyhow::Result;

/// Délai maximal d'un diagnostic : au-delà, `ssh` est tué et le probe échoue.
const PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Ligne émise en tête de sortie : tout ce qui précède (bannière Wallix,
/// motd, sortie des fichiers de démarrage du shell) est ignoré au parsing.
const PROBE_MARKER: &str = "__SUSSHI_PROBE__";

/// Partie fixe du one-liner bash envoyé en argument SSH.
/// Après le marqueur [`PROBE_MARKER`], retourne exactement 7 lignes :
/// 1. kernel (`uname -r`)
/// 2. modèle CPU (première entrée `/proc/cpuinfo`)
/// 3. nombre de cœurs logiques (`nproc`)
/// 4. nom et version de l'OS (`PRETTY_NAME` dans `/etc/os-release`)
/// 5. load average 1/5/15m
/// 6. RAM : `<pct_used> <total_bytes>`
/// 7. Disque `/` : `<pct_used> <total_bytes>`
const PROBE_BASE: &str = concat!(
    "uname -r; ",
    "awk '/^model name/{sub(/.*: /,\"\"); print; exit}' /proc/cpuinfo; ",
    "nproc; ",
    "awk -F= '/^PRETTY_NAME=/{gsub(/\"/,\"\",$2); print $2; exit}' /etc/os-release 2>/dev/null || echo unknown; ",
    "uptime | awk -F'load average:' '{print $2}' | xargs; ",
    "free -b | awk '/^Mem/{printf \"%.0f %.0f\\n\", $3/$2*100, $2}'; ",
    "df -B1 / | awk 'NR==2{printf \"%.0f %.0f\\n\", $3/$2*100, $2}'",
);

/// Construit le one-liner complet en ajoutant une commande `df` par filesystem
/// supplémentaire configuré. Chaque filesystem produit soit `<pct> <bytes>`
/// (présent) soit `absent` (non monté ou inaccessible).
fn build_probe_cmd(extra_filesystems: &[String]) -> String {
    let mut cmd = format!("echo {PROBE_MARKER}; {PROBE_BASE}");
    for fs in extra_filesystems {
        cmd.push_str(&format!(
            "; df -B1 {fs} 2>/dev/null \
            | awk 'NR==2{{printf \"%.0f %.0f\\n\", $3/$2*100, $2; found=1}} \
                   END{{if(!found) print \"absent\"}}'"
        ));
    }
    cmd
}

// ─── Types publics ────────────────────────────────────────────────────────────

/// Usage d'un point de montage retourné par le probe.
#[derive(Debug, Clone)]
pub struct FsUsage {
    /// Pourcentage utilisé (0–100).
    pub pct: u8,
    /// Capacité totale en Go.
    pub total_gb: f32,
}

/// Résultat de l'interrogation d'un filesystem supplémentaire.
#[derive(Debug, Clone)]
pub struct FsEntry {
    /// Chemin du point de montage configuré (ex. `/data`).
    pub mountpoint: String,
    /// `None` si le filesystem est absent ou non monté sur le serveur.
    pub usage: Option<FsUsage>,
}

/// Métriques système collectées par [`probe`].
#[derive(Debug, Clone)]
pub struct ProbeResult {
    pub kernel: String,
    pub cpu_model: String,
    /// Nombre de cœurs logiques (`nproc`).
    pub cpu_cores: u32,
    /// Nom et version de l'OS (ex. `"Debian GNU/Linux 12"`).
    pub os_name: String,
    /// Load average formaté : `"0.42, 0.38, 0.31"`
    pub load: String,
    /// Pourcentage de RAM utilisée (0–100).
    pub ram_pct: u8,
    /// RAM totale en Go.
    pub ram_total_gb: f32,
    /// Pourcentage de disque `/` utilisé (0–100).
    pub disk_pct: u8,
    /// Capacité disque `/` en Go.
    pub disk_total_gb: f32,
    /// Filesystems supplémentaires configurés via `probe_filesystems`.
    pub extra_fs: Vec<FsEntry>,
    /// `true` si un socket ControlMaster SSH actif a été détecté localement.
    pub control_master_active: bool,
}

/// État courant d'un diagnostic lancé sur un serveur.
#[derive(Debug, Default, Clone)]
pub enum ProbeState {
    /// Aucun diagnostic en cours ou résultat effacé.
    #[default]
    Idle,
    /// Diagnostic en attente de réponse SSH.
    Running,
    /// Résultat disponible.
    Done(ProbeResult),
    /// Erreur (message lisible).
    Error(String),
}

// ─── Parsing ──────────────────────────────────────────────────────────────────

impl ProbeResult {
    /// Parse la sortie de `build_probe_cmd` : 7 lignes fixes + N lignes pour
    /// les filesystems supplémentaires (soit `<pct> <bytes>` soit `absent`).
    /// Si le marqueur [`PROBE_MARKER`] est présent, seules les lignes qui le
    /// suivent sont lues.
    pub fn parse(raw: &str, extra_filesystems: &[String]) -> Result<Self> {
        let body = raw
            .lines()
            .position(|l| l.trim() == PROBE_MARKER)
            .map_or(raw.lines().collect::<Vec<_>>(), |i| {
                raw.lines().skip(i + 1).collect()
            });
        let lines: Vec<&str> = body
            .into_iter()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();

        if lines.len() < 7 {
            anyhow::bail!(
                "sortie inattendue ({} lignes au lieu de 7) :\n{}",
                lines.len(),
                raw
            );
        }

        let cpu_cores: u32 = lines[2]
            .parse()
            .map_err(|e| anyhow::anyhow!("cpu_cores: {}", e))?;
        let os_name = lines[3].to_string();
        let (ram_pct, ram_total_gb) = parse_pct_bytes(lines[5], "RAM")?;
        let (disk_pct, disk_total_gb) = parse_pct_bytes(lines[6], "Disk")?;

        let mut extra_fs = Vec::new();
        for (i, mountpoint) in extra_filesystems.iter().enumerate() {
            let usage = match lines.get(7 + i) {
                Some(&"absent") | None => None,
                Some(line) => {
                    let (pct, total_gb) = parse_pct_bytes(line, mountpoint)?;
                    Some(FsUsage { pct, total_gb })
                }
            };
            extra_fs.push(FsEntry {
                mountpoint: mountpoint.clone(),
                usage,
            });
        }

        Ok(ProbeResult {
            kernel: lines[0].to_string(),
            cpu_model: lines[1].to_string(),
            cpu_cores,
            os_name,
            load: lines[4].to_string(),
            ram_pct,
            ram_total_gb,
            disk_pct,
            disk_total_gb,
            extra_fs,
            control_master_active: false,
        })
    }
}

fn parse_pct_bytes(line: &str, label: &str) -> Result<(u8, f32)> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 2 {
        anyhow::bail!("format {} inattendu : {:?}", label, line);
    }
    let pct: u8 = parts[0]
        .parse()
        .map_err(|e| anyhow::anyhow!("{} pct: {}", label, e))?;
    let bytes: u64 = parts[1]
        .parse()
        .map_err(|e| anyhow::anyhow!("{} bytes: {}", label, e))?;
    Ok((pct, bytes as f32 / 1_073_741_824.0))
}

// ─── Probe ────────────────────────────────────────────────────────────────────

/// Lance un diagnostic SSH non-interactif et retourne les métriques système.
///
/// Le one-liner est exécuté via [`run_remote`], qui gère les trois modes
/// (`Direct`, `Jump`, `Wallix` — autorisations candidates comprises).
pub fn probe(server: &ResolvedServer, mode: ConnectionMode) -> Result<ProbeResult> {
    let cmd = build_probe_cmd(&server.probe_filesystems);
    let output = run_remote(server, mode, &cmd, PROBE_TIMEOUT)?;

    // Un bastion peut fermer la session sans transmettre le code de sortie :
    // une sortie complète (marqueur présent) prime sur le statut.
    if !output.success && !output.stdout.contains(PROBE_MARKER) {
        let stderr = output.stderr.trim();
        // Produire des messages lisibles pour les erreurs known_host courantes.
        if stderr.contains("REMOTE HOST IDENTIFICATION HAS CHANGED") {
            anyhow::bail!(
                "La clé SSH de cet hôte a changé.\n\
                 Supprimez l'ancienne entrée avec :\n\
                 ssh-keygen -R {}",
                server.host
            );
        }
        if stderr.contains("Host key verification failed") {
            anyhow::bail!(
                "Vérification de la clé SSH échouée pour {}.\n\
                 Vérifiez ~/.ssh/known_hosts.",
                server.host
            );
        }
        anyhow::bail!("SSH probe échoué : {}", stderr);
    }

    let mut result = ProbeResult::parse(&output.stdout, &server.probe_filesystems)?;
    // ControlMaster non supporté en mode Wallix (voir build_ssh_args).
    result.control_master_active =
        mode != ConnectionMode::Wallix && crate::ssh::client::is_control_master_active(server);
    Ok(result)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Sortie typique du one-liner sur un serveur Debian.
    const SAMPLE: &str = "6.1.0-28-amd64\n\
        Intel(R) Xeon(R) CPU E5-2670 0 @ 2.60GHz\n\
        8\n\
        Debian GNU/Linux 12 (bookworm)\n\
        0.42, 0.38, 0.31\n\
        67 16106127360\n\
        23 499963174912\n";

    #[test]
    fn parse_sets_control_master_active_false_by_default() {
        let r = ProbeResult::parse(SAMPLE, &[]).unwrap();
        assert!(!r.control_master_active);
    }

    #[test]
    fn parse_valid() {
        let r = ProbeResult::parse(SAMPLE, &[]).unwrap();
        assert_eq!(r.kernel, "6.1.0-28-amd64");
        assert_eq!(r.cpu_model, "Intel(R) Xeon(R) CPU E5-2670 0 @ 2.60GHz");
        assert_eq!(r.cpu_cores, 8);
        assert_eq!(r.os_name, "Debian GNU/Linux 12 (bookworm)");
        assert_eq!(r.load, "0.42, 0.38, 0.31");
        assert_eq!(r.ram_pct, 67);
        assert!((r.ram_total_gb - 15.0).abs() < 1.0, "ram ~15 GB");
        assert_eq!(r.disk_pct, 23);
        assert!((r.disk_total_gb - 465.7).abs() < 1.0, "disk ~465 GB");
        assert!(r.extra_fs.is_empty());
    }

    #[test]
    fn parse_ignores_trailing_blank_lines() {
        let with_blanks = format!("{}\n\n", SAMPLE);
        assert!(ProbeResult::parse(&with_blanks, &[]).is_ok());
    }

    #[test]
    fn parse_too_few_lines() {
        let err = ProbeResult::parse("only one line\n", &[]).unwrap_err();
        assert!(err.to_string().contains("lignes"));
    }

    #[test]
    fn parse_bad_ram_pct() {
        let bad = "6.1.0\nIntel\n8\nDebian 12\n0.1\nXX 16000000000\n23 500000000000\n";
        assert!(ProbeResult::parse(bad, &[]).is_err());
    }

    #[test]
    fn parse_bad_ram_bytes() {
        let bad = "6.1.0\nIntel\n8\nDebian 12\n0.1\n67 not_a_number\n23 500000000000\n";
        assert!(ProbeResult::parse(bad, &[]).is_err());
    }

    #[test]
    fn parse_bad_disk_pct() {
        let bad = "6.1.0\nIntel\n8\nDebian 12\n0.1\n67 16000000000\nXX 500000000000\n";
        assert!(ProbeResult::parse(bad, &[]).is_err());
    }

    #[test]
    fn parse_disk_single_column() {
        // Seulement un champ sur la ligne disk → erreur
        let bad = "6.1.0\nIntel\n8\nDebian 12\n0.1\n67 16000000000\n23\n";
        assert!(ProbeResult::parse(bad, &[]).is_err());
    }

    #[test]
    fn parse_bad_cpu_cores() {
        let bad = "6.1.0\nIntel\nnot_a_number\nDebian 12\n0.1\n67 16000000000\n23 500000000000\n";
        assert!(ProbeResult::parse(bad, &[]).is_err());
    }

    #[test]
    fn parse_extra_fs_present() {
        // Sortie avec deux filesystems supplémentaires présents
        let raw = format!("{}{}", SAMPLE, "45 107374182400\n30 53687091200\n");
        let fs_list = vec!["/data".to_string(), "/var/log".to_string()];
        let r = ProbeResult::parse(&raw, &fs_list).unwrap();
        assert_eq!(r.extra_fs.len(), 2);

        let data = &r.extra_fs[0];
        assert_eq!(data.mountpoint, "/data");
        let usage = data.usage.as_ref().expect("/data should be present");
        assert_eq!(usage.pct, 45);
        assert!((usage.total_gb - 100.0).abs() < 1.0, "expected ~100 GB");

        let varlog = &r.extra_fs[1];
        assert_eq!(varlog.mountpoint, "/var/log");
        assert!(varlog.usage.is_some());
    }

    #[test]
    fn parse_extra_fs_absent() {
        // Premier filesystem présent, second absent
        let raw = format!("{}{}", SAMPLE, "45 107374182400\nabsent\n");
        let fs_list = vec!["/data".to_string(), "/backup".to_string()];
        let r = ProbeResult::parse(&raw, &fs_list).unwrap();
        assert_eq!(r.extra_fs.len(), 2);
        assert!(r.extra_fs[0].usage.is_some());
        assert!(r.extra_fs[1].usage.is_none(), "/backup should be absent");
    }

    #[test]
    fn parse_extra_fs_all_absent() {
        // Aucun filesystem supplémentaire dans la sortie (ligne manquante → None)
        let fs_list = vec!["/mnt/nas".to_string()];
        let r = ProbeResult::parse(SAMPLE, &fs_list).unwrap();
        assert_eq!(r.extra_fs.len(), 1);
        assert!(r.extra_fs[0].usage.is_none(), "/mnt/nas should be absent");
    }

    #[test]
    fn build_probe_cmd_no_extra() {
        let cmd = build_probe_cmd(&[]);
        assert!(cmd.contains("uname -r"));
        assert!(!cmd.contains("df -B1 /data"));
    }

    #[test]
    fn build_probe_cmd_with_extra() {
        let fs = vec!["/data".to_string(), "/backup".to_string()];
        let cmd = build_probe_cmd(&fs);
        assert!(cmd.contains("df -B1 /data"));
        assert!(cmd.contains("df -B1 /backup"));
        assert!(cmd.contains("if(!found) print \"absent\""));
    }

    #[test]
    fn wallix_probe_without_bastion_fails() {
        let server = ResolvedServer {
            namespace: String::new(),
            group_name: String::new(),
            env_name: String::new(),
            name: "wallix-srv".to_string(),
            host: "APP-ALPHA-BD".to_string(),
            user: "demo_user".to_string(),
            port: 22,
            ssh_key: String::new(),
            ssh_options: vec![],
            default_mode: ConnectionMode::Wallix,
            jump_host: None,
            bastion_host: None,
            bastion_user: Some("demo_user".to_string()),
            bastion_template: "{target_user}@%n:SSH:{bastion_user}".to_string(),
            wallix_group: Some("APP-ALPHA_ops-admins".to_string()),
            wallix_account: "default".to_string(),
            wallix_protocol: "SSH".to_string(),
            wallix_auto_select: true,
            wallix_fail_if_menu_match_error: true,
            wallix_selection_timeout_secs: 8,
            wallix_direct: false,
            wallix_authorization: None,
            wallix_authorizations: vec![],
            wallix_header_columns: vec![],
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
        };

        // Plus de profil dégradé : le probe Wallix collecte les vraies
        // métriques, et échoue proprement si le bastion manque.
        let err = probe(&server, ConnectionMode::Wallix).unwrap_err();
        assert!(err.to_string().contains("Wallix host"), "{err}");
    }

    /// Bannière (Wallix, motd…) avant la sortie : ignorée grâce au marqueur.
    #[test]
    fn parse_skips_banner_before_marker() {
        let raw = format!("Bienvenue sur le bastion\nUsage restreint\n{PROBE_MARKER}\n{SAMPLE}");
        let r = ProbeResult::parse(&raw, &[]).unwrap();
        assert_eq!(r.kernel, "6.1.0-28-amd64");
        assert_eq!(r.cpu_cores, 8);
    }

    #[test]
    fn build_probe_cmd_starts_with_marker() {
        assert!(build_probe_cmd(&[]).starts_with(&format!("echo {PROBE_MARKER}; ")));
    }
}
