use super::*;
use std::process::Command;

/// Résout la racine du dépôt git contenant `start_dir`, si `start_dir` (ou un de
/// ses parents) se trouve dans un working tree git. Retourne `None` sans erreur
/// si `git` est absent, si le chemin n'est pas un dépôt, ou pour tout autre échec —
/// cette vérification est un bonus best-effort, jamais une condition bloquante.
fn git_repo_toplevel(start_dir: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .arg("-C")
        .arg(start_dir)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    let trimmed = stdout.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

/// Nombre de commits dont `HEAD` est en retard sur sa branche amont (`@{upstream}`),
/// déterminé uniquement à partir des refs déjà connues localement — n'effectue
/// jamais de `git fetch`. Retourne `None` si aucune branche amont n'est configurée
/// ou en cas d'échec.
fn git_behind_count(repo_toplevel: &Path) -> Option<u32> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_toplevel)
        .args(["rev-list", "--count", "HEAD..@{upstream}"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()?
        .trim()
        .parse::<u32>()
        .ok()
}

/// Calcule les avertissements « dépôt git en retard » pour tous les includes
/// locaux résolus dans `config` (namespaces issus de `Config::load_merged`).
/// Un seul avertissement par dépôt git, même si plusieurs includes proviennent
/// du même checkout. Les includes HTTPS sont ignorés (rien à vérifier localement).
pub fn git_outdated_warnings(config: &Config) -> Vec<IncludeWarning> {
    let mut cache: HashMap<PathBuf, Option<u32>> = HashMap::new();
    let mut warned: HashSet<PathBuf> = HashSet::new();
    let mut warnings = Vec::new();

    for entry in &config.groups {
        let ConfigEntry::Namespace(ns) = entry else {
            continue;
        };
        if ns.source_path.starts_with("http://") || ns.source_path.starts_with("https://") {
            continue;
        }
        let Some(parent) = Path::new(&ns.source_path).parent() else {
            continue;
        };
        let Some(toplevel) = git_repo_toplevel(parent) else {
            continue;
        };

        let behind = *cache
            .entry(toplevel.clone())
            .or_insert_with(|| git_behind_count(&toplevel));

        if let Some(behind) = behind
            && behind > 0
            && warned.insert(toplevel.clone())
        {
            warnings.push(IncludeWarning::GitOutdated {
                path: toplevel.display().to_string(),
                behind,
            });
        }
    }

    warnings
}
