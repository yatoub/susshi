use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Mode de connexion SSH. Remplace les chaînes magiques "direct"/"jump"/"wallix".
/// Copy car l'enum ne contient aucune donnée — pas besoin de clone explicite.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionMode {
    #[default]
    Direct,
    Jump,
    /// Anciennement `bastion` — `#[serde(alias)]` conservé pour rétrocompatibilité.
    #[serde(alias = "bastion")]
    Wallix,
}

impl fmt::Display for ConnectionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionMode::Direct => write!(f, "direct"),
            ConnectionMode::Jump => write!(f, "jump"),
            ConnectionMode::Wallix => write!(f, "wallix"),
        }
    }
}

impl ConnectionMode {
    /// Indice tab (Direct=0, Jump=1, Wallix=2) — utilisé par l'UI Tabs::select().
    pub fn index(self) -> usize {
        match self {
            ConnectionMode::Direct => 0,
            ConnectionMode::Jump => 1,
            ConnectionMode::Wallix => 2,
        }
    }

    /// Construit depuis un indice tab. Retourne Direct pour tout indice inconnu.
    pub fn from_index(i: usize) -> Self {
        match i {
            1 => ConnectionMode::Jump,
            2 => ConnectionMode::Wallix,
            _ => ConnectionMode::Direct,
        }
    }

    /// Passe au mode suivant en boucle (Direct → Jump → Wallix → Direct).
    pub fn next(self) -> Self {
        Self::from_index((self.index() + 1) % 3)
    }
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),
    #[error("Missing configuration for server '{0}': {1}")]
    MissingField(String, String),
}

// ─── Multi-fichiers ───────────────────────────────────────────────────────────

/// Entrée dans la section `includes` du YAML principal.
#[derive(Debug, Deserialize, Clone)]
pub struct IncludeEntry {
    pub label: String,
    pub path: String,
    /// Si `true`, les `defaults` du fichier principal sont fusionnés comme
    /// couche de base pour les serveurs du sous-fichier. Défaut : `false`.
    #[serde(default)]
    pub merge_defaults: bool,
}

/// Namespace résolu depuis un fichier inclus — construit programmatiquement,
/// jamais désérialisé depuis le YAML.
#[derive(Debug, Clone)]
pub struct NamespaceEntry {
    pub label: String,
    pub source_path: String,
    /// Defaults locaux du sous-fichier (ne s'appliquent pas au fichier principal).
    pub defaults: Option<Defaults>,
    pub entries: Vec<ConfigEntry>,
    /// Variables `{{ var }}` définies dans le sous-fichier (scope local au fichier).
    pub vars: HashMap<String, String>,
}

// NamespaceEntry doit implémenter Deserialize pour que ConfigEntry puisse le faire
// (derive macros s'appliquent à tout l'enum). Cette impl échoue toujours car les
// namespaces ne proviennent jamais du YAML.
impl<'de> serde::Deserialize<'de> for NamespaceEntry {
    fn deserialize<D: serde::Deserializer<'de>>(_d: D) -> Result<Self, D::Error> {
        Err(serde::de::Error::custom(
            "NamespaceEntry cannot be deserialized from YAML",
        ))
    }
}

/// Avertissement non-bloquant émis lors du chargement multi-fichiers.
#[derive(Debug, Clone)]
pub enum IncludeWarning {
    /// Fichier inclus introuvable ou illisible.
    LoadError {
        label: String,
        path: String,
        error: String,
    },
    /// Dépendance circulaire détectée.
    Circular { label: String, path: String },
}

/// Avertissement émis lors de la validation YAML — champ inconnu ou inattendu.
#[derive(Debug, Clone)]
pub struct ValidationWarning {
    /// Chemin du fichier YAML analysé.
    pub file: String,
    /// Contexte dans la structure YAML (ex. `"defaults"`, `"groups[0].servers[2]"`).
    pub context: String,
    /// Nom du champ inconnu.
    pub field: String,
}

impl fmt::Display for ValidationWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}): champ inconnu \u{00ab} {} \u{00bb}",
            self.file, self.context, self.field
        )
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    pub defaults: Option<Defaults>,
    pub groups: Vec<ConfigEntry>,
    /// Fichiers supplémentaires à fusionner (ignoré dans les sous-fichiers).
    #[serde(default)]
    pub includes: Vec<IncludeEntry>,
    /// Variables de templating `{{ var }}` (scope local au fichier YAML).
    /// Exemple : `_vars: { jump: "bastion.prod.example.com" }`
    /// Usage   : `host: "{{ jump }}"`
    #[serde(default, rename = "_vars")]
    pub vars: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum ConfigEntry {
    Server(Server),
    Group(Group),
    /// Namespace issu d'un fichier inclus — jamais désérialisé directement depuis le YAML.
    Namespace(NamespaceEntry),
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeVariant {
    Latte,
    Frappe,
    Macchiato,
    #[default]
    Mocha,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Defaults {
    pub user: Option<String>,
    pub ssh_key: Option<String>,
    /// Certificat SSH signé à passer avec `-i` (en complément de ssh_key).
    pub ssh_cert: Option<String>,
    /// Socket de l'agent SSH à utiliser pour ce serveur (remplace SSH_AUTH_SOCK).
    pub ssh_agent_sock: Option<String>,
    pub mode: Option<ConnectionMode>,
    pub ssh_port: Option<u16>,
    pub ssh_options: Option<Vec<String>>,
    pub wallix: Option<BastionConfig>,
    pub jump: Option<Vec<JumpConfig>>,
    /// Si `true`, ne passe pas `-F /dev/null` afin de respecter `~/.ssh/config`.
    /// Défaut : `false` (comportement historique).
    pub use_system_ssh_config: Option<bool>,
    /// Variante Catppuccin à utiliser pour le thème TUI.
    /// Valeurs : `latte`, `frappe`, `macchiato`, `mocha` (défaut).
    pub theme: Option<ThemeVariant>,
    /// Points de montage supplémentaires à interroger lors d'un probe.
    pub probe_filesystems: Option<Vec<String>>,
    /// Si `true`, la TUI se rouvre automatiquement après la fermeture d'une connexion SSH.
    /// Défaut : `false` (comportement historique : quitte l'application).
    pub keep_open: Option<bool>,
    /// Tunnels SSH préconfigurés (local-port-forwarding).
    /// Sémantique : REPLACE — un niveau enfant remplace entièrement la liste parente.
    /// Non disponible en mode Wallix.
    pub tunnels: Option<Vec<TunnelConfig>>,
    /// Filtre de recherche actif au démarrage (ex. `"#prod"`).
    pub default_filter: Option<String>,
    /// Tags hérités en cascade par tous les serveurs du périmètre.
    pub tags: Option<Vec<String>>,
    /// Si `true`, active le multiplexage SSH ControlMaster (réutilise la connexion TCP).
    pub control_master: Option<bool>,
    /// Si `true`, active le forwarding de l'agent SSH (`-A`).
    pub agent_forwarding: Option<bool>,
    /// Chemin du socket ControlPath (tilde expandé).
    /// Défaut : `"~/.ssh/ctl/%h_%p_%r"`.
    pub control_path: Option<String>,
    /// Durée de maintien du master après déconnexion. Défaut : `"10m"`.
    pub control_persist: Option<String>,
    /// Chemin vers le script à exécuter avant chaque connexion SSH.
    /// Le hook reçoit : `SUSSHI_SERVER`, `SUSSHI_HOST`, `SUSSHI_USER`, `SUSSHI_PORT`, `SUSSHI_MODE`.
    /// Un code de retour non-zéro annule la connexion.
    pub pre_connect_hook: Option<String>,
    /// Chemin vers le script à exécuter après chaque déconnexion SSH.
    pub post_disconnect_hook: Option<String>,
    /// Délai maximum accordé à un hook avant de le tuer (secondes). Défaut : 5.
    pub hook_timeout_secs: Option<u64>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct BastionConfig {
    pub host: Option<String>,
    pub user: Option<String>,
    pub group: Option<String>,
    pub template: Option<String>,
    /// Compte Wallix (ex: "default"). Défaut : "default".
    pub account: Option<String>,
    /// Protocole Wallix (ex: "SSH"). Défaut : "SSH".
    pub protocol: Option<String>,
    /// Auto-sélectionner l'ID dans le menu Wallix si match unique. Défaut : true.
    pub auto_select: Option<bool>,
    /// Abort si pas de match unique et auto_select=true. Défaut : true.
    pub fail_if_menu_match_error: Option<bool>,
    /// Timeout avant abandon du parsing menu (secondes). Défaut : 8.
    pub selection_timeout_secs: Option<u64>,
    /// Connexion directe sans menu interactif (login filtré bastion_user@host:proto:bastion_user).
    pub direct: Option<bool>,
    /// Nom exact de l'autorisation Wallix (ex: "STI-ANSCORE_ces3s-admins").
    /// Quand défini, inclus dans le login filtré pour forcer la sélection côté Wallix.
    pub authorization: Option<String>,
    /// Tokens de détection d'en-tête dans le menu Wallix (défaut : ["ID", "Cible", "Autorisation"]).
    /// Remplacez si votre bastion affiche des colonnes dans une autre langue.
    pub header_columns: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct JumpConfig {
    pub host: Option<String>,
    pub user: Option<String>,
}

/// Configuration d'un tunnel SSH local-port-forwarding.
/// Chaque entrée produit : `ssh -L local_port:remote_host:remote_port -N`
///
/// Non disponible en mode Wallix.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct TunnelConfig {
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
    /// Label affiché dans l'UI (optionnel — auto-généré si absent).
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Group {
    pub name: String,
    pub user: Option<String>,
    pub ssh_key: Option<String>,
    pub mode: Option<ConnectionMode>,
    pub ssh_port: Option<u16>,
    pub ssh_options: Option<Vec<String>>,
    pub wallix: Option<BastionConfig>,
    pub wallix_group: Option<String>,
    pub jump: Option<Vec<JumpConfig>>,
    pub environments: Option<Vec<Environment>>,
    pub servers: Option<Vec<Server>>,
    pub probe_filesystems: Option<Vec<String>>,
    pub tunnels: Option<Vec<TunnelConfig>>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Environment {
    pub name: String,
    pub user: Option<String>,
    pub ssh_key: Option<String>,
    pub mode: Option<ConnectionMode>,
    pub ssh_port: Option<u16>,
    pub ssh_options: Option<Vec<String>>,
    pub wallix: Option<BastionConfig>,
    pub wallix_group: Option<String>,
    pub jump: Option<Vec<JumpConfig>>,
    pub servers: Vec<Server>,
    pub probe_filesystems: Option<Vec<String>>,
    pub tunnels: Option<Vec<TunnelConfig>>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Server {
    pub name: String,
    pub host: String, // Host is mandatory on leaf
    pub user: Option<String>,
    pub ssh_key: Option<String>,
    /// Certificat SSH signé (complément de ssh_key).
    pub ssh_cert: Option<String>,
    /// Socket de l'agent SSH à utiliser pour ce serveur.
    pub ssh_agent_sock: Option<String>,
    pub ssh_port: Option<u16>,
    pub ssh_options: Option<Vec<String>>,
    pub mode: Option<ConnectionMode>,
    pub wallix: Option<BastionConfig>,
    pub wallix_group: Option<String>,
    pub jump: Option<Vec<JumpConfig>>,
    pub probe_filesystems: Option<Vec<String>>,
    pub tunnels: Option<Vec<TunnelConfig>>,
    pub tags: Option<Vec<String>>,
    /// Script pré-connexion spécifique au serveur (surcharge le défaut).
    pub pre_connect_hook: Option<String>,
    /// Script post-déconnexion spécifique au serveur (surcharge le défaut).
    pub post_disconnect_hook: Option<String>,
    /// Description libre affichée dans le panneau de détail.
    pub notes: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedServer {
    /// Label du namespace (fichier inclus) dont provient ce serveur.
    /// Vide pour les serveurs du fichier principal.
    pub namespace: String,
    pub group_name: String,
    pub env_name: String,
    pub name: String,
    pub host: String,
    pub user: String,
    pub port: u16,
    pub ssh_key: String,
    /// Certificat SSH signé (vide = non configuré).
    pub ssh_cert: String,
    /// Socket de l'agent SSH (vide = utiliser SSH_AUTH_SOCK système).
    pub ssh_agent_sock: String,
    pub ssh_options: Vec<String>,
    pub default_mode: ConnectionMode,
    /// Chaîne prête à passer à `-J` : `"user1@host1:port,user2@host2"` pour un ou plusieurs sauts.
    pub jump_host: Option<String>,
    pub bastion_host: Option<String>,
    pub bastion_user: Option<String>,
    pub bastion_template: String,
    /// Groupe/autorisation Wallix pour la sélection de menu (optionnel).
    pub wallix_group: Option<String>,
    /// Compte Wallix (défaut: "default").
    pub wallix_account: String,
    /// Protocole Wallix (défaut: "SSH").
    pub wallix_protocol: String,
    /// Auto-sélectionner l'ID dans le menu Wallix si match unique.
    pub wallix_auto_select: bool,
    /// Abort si pas de match unique et auto_select=true.
    pub wallix_fail_if_menu_match_error: bool,
    /// Timeout avant abandon du parsing menu (secondes).
    pub wallix_selection_timeout_secs: u64,
    /// Connexion directe sans menu (bypass du probe PTY).
    pub wallix_direct: bool,
    /// Nom exact de l'autorisation Wallix — inclus dans le login filtré quand défini.
    pub wallix_authorization: Option<String>,
    /// Tokens de détection d'en-tête du menu Wallix (vide = defaults : "ID", "Cible", "Autorisation").
    pub wallix_header_columns: Vec<String>,
    /// Respecte `~/.ssh/config` si `true` (ne passe pas `-F /dev/null`).
    pub use_system_ssh_config: bool,
    /// Points de montage à interroger lors d'un probe (hérités en cascade).
    pub probe_filesystems: Vec<String>,
    /// Tunnels SSH préconfigurés (fusion REPLACE depuis la hiérarchie config + overrides state).
    pub tunnels: Vec<TunnelConfig>,
    /// Tags du serveur (union de tous les niveaux : defaults → groupe → env → serveur).
    pub tags: Vec<String>,
    /// Multiplexage SSH ControlMaster actif pour ce serveur.
    pub control_master: bool,
    /// Forwarding de l'agent SSH actif pour ce serveur.
    pub agent_forwarding: bool,
    /// Chemin du socket ControlPath (vide si désactivé).
    pub control_path: String,
    /// Valeur de ControlPersist (ex. `"10m"`).
    pub control_persist: String,
    /// Script pré-connexion (None = désactivé).
    pub pre_connect_hook: Option<String>,
    /// Script post-déconnexion (None = désactivé).
    pub post_disconnect_hook: Option<String>,
    /// Timeout des hooks en secondes.
    pub hook_timeout_secs: u64,
    /// Description libre (champ `notes` du YAML).
    pub notes: String,
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let mut config: Config = serde_yaml_ng::from_str(&content)?;
        config.sort();
        Ok(config)
    }

    /// Alias pratique pour les tests et cas où seul le `Config` est nécessaire.
    /// Utiliser `load_merged` en production pour obtenir les avertissements de validation.
    #[cfg(test)]
    pub fn load_simple<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        Self::load(path)
    }

    pub fn sort(&mut self) {
        // Sort top-level entries (Groups, Servers, Namespaces)
        self.groups.sort_by(|a, b| {
            let name_a = match a {
                ConfigEntry::Group(g) => g.name.as_str(),
                ConfigEntry::Server(s) => s.name.as_str(),
                ConfigEntry::Namespace(ns) => ns.label.as_str(),
            };
            let name_b = match b {
                ConfigEntry::Group(g) => g.name.as_str(),
                ConfigEntry::Server(s) => s.name.as_str(),
                ConfigEntry::Namespace(ns) => ns.label.as_str(),
            };
            name_a.cmp(name_b)
        });

        // Sort children
        for entry in &mut self.groups {
            match entry {
                ConfigEntry::Group(group) => sort_group(group),
                ConfigEntry::Namespace(ns) => {
                    ns.entries.sort_by(|a, b| {
                        let name_a = match a {
                            ConfigEntry::Group(g) => g.name.as_str(),
                            ConfigEntry::Server(s) => s.name.as_str(),
                            ConfigEntry::Namespace(n) => n.label.as_str(),
                        };
                        let name_b = match b {
                            ConfigEntry::Group(g) => g.name.as_str(),
                            ConfigEntry::Server(s) => s.name.as_str(),
                            ConfigEntry::Namespace(n) => n.label.as_str(),
                        };
                        name_a.cmp(name_b)
                    });
                    for sub_entry in &mut ns.entries {
                        if let ConfigEntry::Group(group) = sub_entry {
                            sort_group(group);
                        }
                    }
                }
                ConfigEntry::Server(_) => {}
            }
        }
    }

    pub fn resolve(&self) -> Result<Vec<ResolvedServer>, ConfigError> {
        let mut resolved = Vec::new();

        let d = self.defaults.clone().unwrap_or_default();
        let use_sys_cfg = d.use_system_ssh_config.unwrap_or(false);

        for entry in &self.groups {
            match entry {
                ConfigEntry::Namespace(ns) => {
                    // Les defaults du fichier principal servent de base ; les defaults
                    // locaux du namespace (sous-fichier) les surchargent champ par champ.
                    let ns_local = ns.defaults.clone().unwrap_or_default();
                    let ns_d = merge_default_structs(&d, &ns_local);
                    let ns_use_sys_cfg = ns_d.use_system_ssh_config.unwrap_or(use_sys_cfg);
                    resolve_entries(
                        &ns.entries,
                        &ns_d,
                        ns_use_sys_cfg,
                        &ns.label,
                        &mut resolved,
                        &ns.vars,
                    )?;
                }
                _ => {
                    resolve_entries(
                        std::slice::from_ref(entry),
                        &d,
                        use_sys_cfg,
                        "",
                        &mut resolved,
                        &self.vars,
                    )?;
                }
            }
        }

        Ok(resolved)
    }

    /// Charge le fichier principal et résout tous les `includes` récursivement.
    ///
    /// Retourne le `Config` fusionné, les avertissements d'includes non-bloquants
    /// et les avertissements de validation YAML (champs inconnus).
    ///
    /// `loading_stack` sert à détecter les cycles ; passer `&mut HashSet::new()`
    /// à l'appel de premier niveau.
    pub fn load_merged<P: AsRef<Path>>(
        path: P,
        loading_stack: &mut HashSet<PathBuf>,
    ) -> Result<(Self, Vec<IncludeWarning>, Vec<ValidationWarning>), ConfigError> {
        let path = path.as_ref();
        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

        // Lecture du contenu pour la validation ET le parsing.
        let content = std::fs::read_to_string(path)?;
        let mut config: Config = serde_yaml_ng::from_str(&content)?;
        config.sort();

        let mut inc_warnings: Vec<IncludeWarning> = Vec::new();
        let mut val_warnings: Vec<ValidationWarning> =
            validate_yaml(&content, &canonical.display().to_string());

        if config.includes.is_empty() {
            return Ok((config, inc_warnings, val_warnings));
        }

        loading_stack.insert(canonical.clone());
        let parent_dir = canonical
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf();
        let includes = std::mem::take(&mut config.includes);
        let main_defaults = config.defaults.clone().unwrap_or_default();

        for entry in includes {
            // ── Include URL HTTPS ────────────────────────────────────────────
            if entry.path.starts_with("https://") || entry.path.starts_with("http://") {
                let url = entry.path.clone();
                let content = match fetch_url(&url) {
                    Ok(c) => c,
                    Err(e) => {
                        inc_warnings.push(IncludeWarning::LoadError {
                            label: entry.label.clone(),
                            path: url.clone(),
                            error: e,
                        });
                        continue;
                    }
                };
                let mut sub_config: Config = match serde_yaml_ng::from_str(&content) {
                    Ok(c) => c,
                    Err(e) => {
                        inc_warnings.push(IncludeWarning::LoadError {
                            label: entry.label.clone(),
                            path: url.clone(),
                            error: e.to_string(),
                        });
                        continue;
                    }
                };
                sub_config.sort();
                let sub_val = validate_yaml(&content, &url);
                val_warnings.extend(sub_val);
                if entry.merge_defaults {
                    let sub_d = sub_config.defaults.unwrap_or_default();
                    sub_config.defaults = Some(merge_default_structs(&main_defaults, &sub_d));
                }
                let mut direct_entries = Vec::new();
                let mut nested_namespaces: Vec<NamespaceEntry> = Vec::new();
                for sub_entry in sub_config.groups {
                    match sub_entry {
                        ConfigEntry::Namespace(ns) => nested_namespaces.push(ns),
                        other => direct_entries.push(other),
                    }
                }
                config.groups.push(ConfigEntry::Namespace(NamespaceEntry {
                    label: entry.label.clone(),
                    source_path: url.clone(),
                    defaults: sub_config.defaults,
                    entries: direct_entries,
                    vars: sub_config.vars,
                }));
                for nested in nested_namespaces {
                    config.groups.push(ConfigEntry::Namespace(nested));
                }
                continue;
            }

            // ── Include fichier local ────────────────────────────────────────
            // Résolution du chemin (tilde + relatif)
            let expanded = shellexpand::tilde(&entry.path).into_owned();
            let raw = std::path::Path::new(&expanded);
            let sub_path = if raw.is_absolute() {
                raw.to_path_buf()
            } else {
                parent_dir.join(raw)
            };

            // Forme canonique pour la détection de cycle
            let sub_canonical = match std::fs::canonicalize(&sub_path) {
                Ok(p) => p,
                Err(e) => {
                    inc_warnings.push(IncludeWarning::LoadError {
                        label: entry.label.clone(),
                        path: sub_path.display().to_string(),
                        error: e.to_string(),
                    });
                    continue;
                }
            };

            if loading_stack.contains(&sub_canonical) {
                inc_warnings.push(IncludeWarning::Circular {
                    label: entry.label.clone(),
                    path: sub_canonical.display().to_string(),
                });
                continue;
            }

            // Chargement récursif du sous-fichier
            let (mut sub_config, mut sub_inc, sub_val) =
                match Self::load_merged(&sub_path, loading_stack) {
                    Ok(r) => r,
                    Err(e) => {
                        inc_warnings.push(IncludeWarning::LoadError {
                            label: entry.label.clone(),
                            path: sub_path.display().to_string(),
                            error: e.to_string(),
                        });
                        continue;
                    }
                };
            inc_warnings.append(&mut sub_inc);
            val_warnings.extend(sub_val);

            // Fusion optionnelle des defaults du fichier principal
            if entry.merge_defaults {
                let sub_d = sub_config.defaults.unwrap_or_default();
                sub_config.defaults = Some(merge_default_structs(&main_defaults, &sub_d));
            }

            // Partition : entrées directes vs namespaces imbriqués (issus d'includes récursifs)
            let mut direct_entries = Vec::new();
            let mut nested_namespaces: Vec<NamespaceEntry> = Vec::new();
            for sub_entry in sub_config.groups {
                match sub_entry {
                    ConfigEntry::Namespace(ns) => nested_namespaces.push(ns),
                    other => direct_entries.push(other),
                }
            }

            // Namespace principal avec les entrées directes
            config.groups.push(ConfigEntry::Namespace(NamespaceEntry {
                label: entry.label.clone(),
                source_path: sub_canonical.display().to_string(),
                defaults: sub_config.defaults,
                entries: direct_entries,
                vars: sub_config.vars.clone(),
            }));

            // Namespaces imbriqués aplatis avec label préfixé "parent / enfant"
            for nested in nested_namespaces {
                config.groups.push(ConfigEntry::Namespace(NamespaceEntry {
                    label: format!("{} / {}", entry.label, nested.label),
                    source_path: nested.source_path,
                    defaults: nested.defaults,
                    entries: nested.entries,
                    vars: nested.vars,
                }));
            }
        }

        loading_stack.remove(&canonical);
        config.sort();

        Ok((config, inc_warnings, val_warnings))
    }
}

/// Résout un slice d'entrées de configuration avec les defaults et le namespace donnés.
fn resolve_entries(
    entries: &[ConfigEntry],
    d: &Defaults,
    use_sys_cfg: bool,
    namespace: &str,
    resolved: &mut Vec<ResolvedServer>,
    vars: &HashMap<String, String>,
) -> Result<(), ConfigError> {
    for entry in entries {
        match entry {
            ConfigEntry::Group(group) => {
                // Merge defaults -> Group
                let g_user = group.user.as_deref().or(d.user.as_deref());
                let g_key = group.ssh_key.as_deref().or(d.ssh_key.as_deref());
                let g_mode = group.mode.or(d.mode);
                let g_port = group.ssh_port.or(d.ssh_port);
                let g_opts = if let Some(opts) = &group.ssh_options {
                    Some(opts.clone())
                } else {
                    d.ssh_options.clone()
                };
                let g_fs = extend_filesystems(
                    d.probe_filesystems.as_ref(),
                    group.probe_filesystems.as_ref(),
                );

                let g_bastion = merge_bastion(&d.wallix, &group.wallix);
                let g_jump = merge_jump(&d.jump, &group.jump);
                let g_tunnels = replace_tunnels(&d.tunnels, &group.tunnels);
                let g_tags = extend_tags(d.tags.as_ref(), group.tags.as_ref());

                if let Some(envs) = &group.environments {
                    for env in envs {
                        // Merge Group -> Env
                        let e_user = env.user.as_deref().or(g_user);
                        let e_key = env.ssh_key.as_deref().or(g_key);
                        let e_mode = env.mode.or(g_mode);
                        let e_port = env.ssh_port.or(g_port);
                        let e_opts = if let Some(opts) = &env.ssh_options {
                            Some(opts.clone())
                        } else {
                            g_opts.clone()
                        };
                        let e_fs = extend_filesystems(
                            g_fs.as_ref().map(|v| v as &Vec<String>),
                            env.probe_filesystems.as_ref(),
                        );

                        let e_bastion = merge_bastion(&g_bastion, &env.wallix);
                        let e_jump = merge_jump(&g_jump, &env.jump);
                        let e_tunnels = replace_tunnels(&g_tunnels, &env.tunnels);
                        let e_tags = extend_tags(Some(&g_tags), env.tags.as_ref());

                        let env_def = ServerDefaults {
                            user: e_user,
                            key: e_key,
                            cert: d.ssh_cert.as_deref(),
                            agent_sock: d.ssh_agent_sock.as_deref(),
                            mode: e_mode,
                            port: e_port,
                            opts: e_opts.as_ref(),
                            bastion: &e_bastion,
                            jump: &e_jump,
                            use_system_ssh_config: use_sys_cfg,
                            fs: e_fs.clone(),
                            tunnels: e_tunnels.as_ref(),
                            namespace,
                            vars,
                            control_master: d.control_master.unwrap_or(false),
                            agent_forwarding: d.agent_forwarding.unwrap_or(false),
                            control_path: d
                                .control_path
                                .as_deref()
                                .unwrap_or("~/.ssh/ctl/%h_%p_%r"),
                            control_persist: d.control_persist.as_deref().unwrap_or("10m"),
                            pre_connect_hook: d.pre_connect_hook.as_deref(),
                            post_disconnect_hook: d.post_disconnect_hook.as_deref(),
                            hook_timeout_secs: d.hook_timeout_secs.unwrap_or(5),
                            tags: e_tags,
                        };
                        for server in &env.servers {
                            let r = resolve_server(server, &group.name, &env.name, &env_def)?;
                            resolved.push(r);
                        }
                    }
                }

                if let Some(servers) = &group.servers {
                    let grp_def = ServerDefaults {
                        user: g_user,
                        key: g_key,
                        cert: d.ssh_cert.as_deref(),
                        agent_sock: d.ssh_agent_sock.as_deref(),
                        mode: g_mode,
                        port: g_port,
                        opts: g_opts.as_ref(),
                        bastion: &g_bastion,
                        jump: &g_jump,
                        use_system_ssh_config: use_sys_cfg,
                        fs: g_fs.clone(),
                        tunnels: g_tunnels.as_ref(),
                        namespace,
                        vars,
                        control_master: d.control_master.unwrap_or(false),
                        agent_forwarding: d.agent_forwarding.unwrap_or(false),
                        control_path: d.control_path.as_deref().unwrap_or("~/.ssh/ctl/%h_%p_%r"),
                        control_persist: d.control_persist.as_deref().unwrap_or("10m"),
                        pre_connect_hook: d.pre_connect_hook.as_deref(),
                        post_disconnect_hook: d.post_disconnect_hook.as_deref(),
                        hook_timeout_secs: d.hook_timeout_secs.unwrap_or(5),
                        tags: g_tags.clone(),
                    };
                    for server in servers {
                        let r = resolve_server(server, &group.name, "", &grp_def)?;
                        resolved.push(r);
                    }
                }
            }
            ConfigEntry::Server(server) => {
                // Top-level server (root ou dans un namespace)
                let top_def = ServerDefaults {
                    user: d.user.as_deref(),
                    key: d.ssh_key.as_deref(),
                    cert: d.ssh_cert.as_deref(),
                    agent_sock: d.ssh_agent_sock.as_deref(),
                    mode: d.mode,
                    port: d.ssh_port,
                    opts: d.ssh_options.as_ref(),
                    bastion: &d.wallix,
                    jump: &d.jump,
                    use_system_ssh_config: use_sys_cfg,
                    fs: d.probe_filesystems.clone(),
                    tunnels: d.tunnels.as_ref(),
                    namespace,
                    vars,
                    control_master: d.control_master.unwrap_or(false),
                    agent_forwarding: d.agent_forwarding.unwrap_or(false),
                    control_path: d.control_path.as_deref().unwrap_or("~/.ssh/ctl/%h_%p_%r"),
                    control_persist: d.control_persist.as_deref().unwrap_or("10m"),
                    pre_connect_hook: d.pre_connect_hook.as_deref(),
                    post_disconnect_hook: d.post_disconnect_hook.as_deref(),
                    hook_timeout_secs: d.hook_timeout_secs.unwrap_or(5),
                    tags: extend_tags(None, d.tags.as_ref()),
                };
                let r = resolve_server(server, "", "", &top_def)?;
                resolved.push(r);
            }
            // Les namespaces imbriqués dans ns.entries ne sont jamais générés
            // après aplatissement dans load_merged — ce bras ne devrait pas être atteint.
            ConfigEntry::Namespace(_) => {}
        }
    }
    Ok(())
}

fn merge_bastion(
    parent: &Option<BastionConfig>,
    child: &Option<BastionConfig>,
) -> Option<BastionConfig> {
    match (parent, child) {
        (None, None) => None,
        (Some(p), None) => Some(p.clone()),
        (None, Some(c)) => Some(c.clone()),
        (Some(p), Some(c)) => Some(BastionConfig {
            host: c.host.clone().or(p.host.clone()),
            user: c.user.clone().or(p.user.clone()),
            group: c.group.clone().or(p.group.clone()),
            template: c.template.clone().or(p.template.clone()),
            account: c.account.clone().or(p.account.clone()),
            protocol: c.protocol.clone().or(p.protocol.clone()),
            auto_select: c.auto_select.or(p.auto_select),
            fail_if_menu_match_error: c.fail_if_menu_match_error.or(p.fail_if_menu_match_error),
            selection_timeout_secs: c.selection_timeout_secs.or(p.selection_timeout_secs),
            direct: c.direct.or(p.direct),
            authorization: c.authorization.clone().or(p.authorization.clone()),
            header_columns: c.header_columns.clone().or(p.header_columns.clone()),
        }),
    }
}

/// Le niveau enfant remplace entièrement le niveau parent (pas de fusion champ par champ),
/// ce qui permet de définir une chaîne de sauts complète à chaque niveau.
fn merge_jump(
    parent: &Option<Vec<JumpConfig>>,
    child: &Option<Vec<JumpConfig>>,
) -> Option<Vec<JumpConfig>> {
    child.clone().or_else(|| parent.clone())
}

/// Même sémantique que `merge_jump` : le niveau enfant remplace la liste parente en entier.
/// Contrairement à `extend_filesystems`, les tunnels ne s'accumulent pas.
fn replace_tunnels(
    parent: &Option<Vec<TunnelConfig>>,
    child: &Option<Vec<TunnelConfig>>,
) -> Option<Vec<TunnelConfig>> {
    child.clone().or_else(|| parent.clone())
}

/// Remplace les occurrences `{{ var }}` dans `s` par les valeurs de `vars`.
/// Les variables non définies sont laissées telles quelles (`{{ var }}`).
pub fn interpolate(s: &str, vars: &HashMap<String, String>) -> String {
    let mut result = s.to_string();
    for (key, value) in vars {
        let placeholder = format!("{{{{ {key} }}}}");
        result = result.replace(&placeholder, value);
    }
    result
}

/// Retourne les noms des variables `{{ var }}` présentes dans `s` mais absentes de `vars`.
pub fn undefined_vars(s: &str, vars: &HashMap<String, String>) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = s;
    while let Some(start) = rest.find("{{") {
        rest = &rest[start + 2..];
        if let Some(end) = rest.find("}}") {
            let inner = rest[..end].trim();
            if !inner.is_empty() && !vars.contains_key(inner) {
                found.push(inner.to_string());
            }
            rest = &rest[end + 2..];
        } else {
            break;
        }
    }
    found
}

/// Les tags s'accumulent : chaque niveau **ajoute** ses tags à ceux du niveau parent.
/// Un serveur hérite donc des tags définis dans les defaults, le groupe et l'environnement.
fn extend_tags(parent: Option<&Vec<String>>, child: Option<&Vec<String>>) -> Vec<String> {
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut merged: Vec<String> = Vec::new();
    for tag in parent
        .into_iter()
        .flatten()
        .chain(child.into_iter().flatten())
    {
        if seen.insert(tag.as_str()) {
            merged.push(tag.clone());
        }
    }
    merged
}

/// Les probe_filesystems s'accumulent en cascade : chaque niveau ajoute ses
/// entrées à celles du niveau parent (sans doublon).
/// Un groupe définissant `/kafka_data` héritera donc aussi des filesystems
/// déclarés dans les defaults.
fn extend_filesystems(
    parent: Option<&Vec<String>>,
    child: Option<&Vec<String>>,
) -> Option<Vec<String>> {
    if parent.is_none() && child.is_none() {
        return None;
    }
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut merged: Vec<String> = Vec::new();
    for item in parent
        .into_iter()
        .flatten()
        .chain(child.into_iter().flatten())
    {
        if seen.insert(item.as_str()) {
            merged.push(item.clone());
        }
    }
    Some(merged)
}

/// Fusionne deux `Defaults` : `overrides` prime sur `base` pour chaque champ `Option`.
/// Utilisé par `load_merged` quand `merge_defaults: true` est activé sur un include.
fn merge_default_structs(base: &Defaults, overrides: &Defaults) -> Defaults {
    Defaults {
        user: overrides.user.clone().or_else(|| base.user.clone()),
        ssh_key: overrides.ssh_key.clone().or_else(|| base.ssh_key.clone()),
        ssh_cert: overrides.ssh_cert.clone().or_else(|| base.ssh_cert.clone()),
        ssh_agent_sock: overrides
            .ssh_agent_sock
            .clone()
            .or_else(|| base.ssh_agent_sock.clone()),
        mode: overrides.mode.or(base.mode),
        ssh_port: overrides.ssh_port.or(base.ssh_port),
        ssh_options: overrides
            .ssh_options
            .clone()
            .or_else(|| base.ssh_options.clone()),
        wallix: overrides.wallix.clone().or_else(|| base.wallix.clone()),
        jump: overrides.jump.clone().or_else(|| base.jump.clone()),
        use_system_ssh_config: overrides
            .use_system_ssh_config
            .or(base.use_system_ssh_config),
        theme: overrides.theme.or(base.theme),
        probe_filesystems: overrides
            .probe_filesystems
            .clone()
            .or_else(|| base.probe_filesystems.clone()),
        keep_open: overrides.keep_open.or(base.keep_open),
        tunnels: overrides.tunnels.clone().or_else(|| base.tunnels.clone()),
        default_filter: overrides
            .default_filter
            .clone()
            .or_else(|| base.default_filter.clone()),
        tags: match (&base.tags, &overrides.tags) {
            (None, r) => r.clone(),
            (l, None) => l.clone(),
            (Some(b), Some(o)) => Some(extend_tags(Some(b), Some(o))),
        },
        control_master: overrides.control_master.or(base.control_master),
        agent_forwarding: overrides.agent_forwarding.or(base.agent_forwarding),
        control_path: overrides
            .control_path
            .clone()
            .or_else(|| base.control_path.clone()),
        control_persist: overrides
            .control_persist
            .clone()
            .or_else(|| base.control_persist.clone()),
        pre_connect_hook: overrides
            .pre_connect_hook
            .clone()
            .or_else(|| base.pre_connect_hook.clone()),
        post_disconnect_hook: overrides
            .post_disconnect_hook
            .clone()
            .or_else(|| base.post_disconnect_hook.clone()),
        hook_timeout_secs: overrides.hook_timeout_secs.or(base.hook_timeout_secs),
    }
}

// ─── Validation YAML ─────────────────────────────────────────────────────────

/// Télécharge le contenu d'une URL HTTPS (ou HTTP) et le retourne sous forme de `String`.
fn fetch_url(url: &str) -> Result<String, String> {
    ureq::get(url)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())
}

/// Analyse `content` (YAML texte) et retourne les avertissements pour tout champ
/// dont le nom ne figure pas dans la liste des clés connues du schéma susshi.
pub fn validate_yaml(content: &str, file_path: &str) -> Vec<ValidationWarning> {
    let value: serde_yaml_ng::Value = match serde_yaml_ng::from_str(content) {
        Ok(v) => v,
        Err(_) => return vec![], // l'erreur de parsing est déjà remontée par serde
    };

    let mut warnings = Vec::new();

    if let serde_yaml_ng::Value::Mapping(root) = &value {
        yaml_check_keys(
            root,
            &["defaults", "groups", "includes", "_vars"],
            file_path,
            "root",
            &mut warnings,
        );

        if let Some(serde_yaml_ng::Value::Mapping(m)) = root.get("defaults") {
            yaml_check_keys(
                m,
                &[
                    "user",
                    "ssh_key",
                    "mode",
                    "ssh_port",
                    "ssh_options",
                    "wallix",
                    "jump",
                    "use_system_ssh_config",
                    "theme",
                    "probe_filesystems",
                    "keep_open",
                    "tunnels",
                    "default_filter",
                    "tags",
                    "control_master",
                    "control_path",
                    "control_persist",
                    "pre_connect_hook",
                    "post_disconnect_hook",
                    "hook_timeout_secs",
                ],
                file_path,
                "defaults",
                &mut warnings,
            );
        }

        if let Some(serde_yaml_ng::Value::Sequence(incs)) = root.get("includes") {
            for (i, inc) in incs.iter().enumerate() {
                if let serde_yaml_ng::Value::Mapping(m) = inc {
                    yaml_check_keys(
                        m,
                        &["label", "path", "merge_defaults"],
                        file_path,
                        &format!("includes[{i}]"),
                        &mut warnings,
                    );
                }
            }
        }

        if let Some(serde_yaml_ng::Value::Sequence(groups)) = root.get("groups") {
            for (i, g) in groups.iter().enumerate() {
                yaml_validate_entry(g, file_path, &format!("groups[{i}]"), &mut warnings);
            }
        }
    }

    warnings
}

fn yaml_validate_entry(
    val: &serde_yaml_ng::Value,
    file: &str,
    ctx: &str,
    warnings: &mut Vec<ValidationWarning>,
) {
    let serde_yaml_ng::Value::Mapping(m) = val else {
        return;
    };
    let has_host = m.contains_key(serde_yaml_ng::Value::String("host".into()));
    let has_envs = m.contains_key(serde_yaml_ng::Value::String("environments".into()));

    if has_host && !has_envs {
        // Serveur
        yaml_check_keys(
            m,
            &[
                "name",
                "host",
                "user",
                "ssh_key",
                "ssh_port",
                "ssh_options",
                "mode",
                "wallix",
                "wallix_group",
                "jump",
                "probe_filesystems",
                "tunnels",
                "tags",
            ],
            file,
            ctx,
            warnings,
        );
    } else {
        // Groupe
        yaml_check_keys(
            m,
            &[
                "name",
                "user",
                "ssh_key",
                "mode",
                "ssh_port",
                "ssh_options",
                "wallix",
                "wallix_group",
                "jump",
                "environments",
                "servers",
                "probe_filesystems",
                "tunnels",
                "tags",
            ],
            file,
            ctx,
            warnings,
        );

        if let Some(serde_yaml_ng::Value::Sequence(envs)) =
            m.get(serde_yaml_ng::Value::String("environments".into()))
        {
            for (i, env) in envs.iter().enumerate() {
                if let serde_yaml_ng::Value::Mapping(em) = env {
                    yaml_check_keys(
                        em,
                        &[
                            "name",
                            "user",
                            "ssh_key",
                            "mode",
                            "ssh_port",
                            "ssh_options",
                            "wallix",
                            "wallix_group",
                            "jump",
                            "servers",
                            "probe_filesystems",
                            "tunnels",
                            "tags",
                        ],
                        file,
                        &format!("{ctx}.environments[{i}]"),
                        warnings,
                    );
                    if let Some(serde_yaml_ng::Value::Sequence(svs)) =
                        em.get(serde_yaml_ng::Value::String("servers".into()))
                    {
                        for (j, s) in svs.iter().enumerate() {
                            yaml_validate_entry(
                                s,
                                file,
                                &format!("{ctx}.environments[{i}].servers[{j}]"),
                                warnings,
                            );
                        }
                    }
                }
            }
        }

        if let Some(serde_yaml_ng::Value::Sequence(svs)) =
            m.get(serde_yaml_ng::Value::String("servers".into()))
        {
            for (j, s) in svs.iter().enumerate() {
                yaml_validate_entry(s, file, &format!("{ctx}.servers[{j}]"), warnings);
            }
        }
    }
}

fn yaml_check_keys(
    m: &serde_yaml_ng::Mapping,
    known: &[&str],
    file: &str,
    ctx: &str,
    warnings: &mut Vec<ValidationWarning>,
) {
    for key in m.keys() {
        if let serde_yaml_ng::Value::String(k) = key
            && !known.contains(&k.as_str())
        {
            warnings.push(ValidationWarning {
                file: file.to_string(),
                context: ctx.to_string(),
                field: k.clone(),
            });
        }
    }
}

/// Trie les environnements et serveurs d'un groupe.
fn sort_group(group: &mut Group) {
    if let Some(envs) = &mut group.environments {
        envs.sort_by(|a, b| a.name.cmp(&b.name));
        for env in envs.iter_mut() {
            env.servers.sort_by(|a, b| a.name.cmp(&b.name));
        }
    }
    if let Some(servers) = &mut group.servers {
        servers.sort_by(|a, b| a.name.cmp(&b.name));
    }
}

/// Defaults effectifs à passer à `resolve_server`, après cascade group→env.
struct ServerDefaults<'a> {
    user: Option<&'a str>,
    key: Option<&'a str>,
    cert: Option<&'a str>,
    agent_sock: Option<&'a str>,
    mode: Option<ConnectionMode>,
    port: Option<u16>,
    opts: Option<&'a Vec<String>>,
    bastion: &'a Option<BastionConfig>,
    jump: &'a Option<Vec<JumpConfig>>,
    use_system_ssh_config: bool,
    fs: Option<Vec<String>>,
    tunnels: Option<&'a Vec<TunnelConfig>>,
    namespace: &'a str,
    vars: &'a HashMap<String, String>,
    control_master: bool,
    agent_forwarding: bool,
    control_path: &'a str,
    control_persist: &'a str,
    pre_connect_hook: Option<&'a str>,
    post_disconnect_hook: Option<&'a str>,
    hook_timeout_secs: u64,
    /// Tags accumulés depuis defaults → group → env (union sans doublon).
    tags: Vec<String>,
}

fn resolve_server(
    s: &Server,
    group: &str,
    env: &str,
    def: &ServerDefaults<'_>,
) -> Result<ResolvedServer, ConfigError> {
    let def_user = def.user;
    let def_key = def.key;
    let def_cert = def.cert;
    let def_agent_sock = def.agent_sock;
    let def_mode = def.mode;
    let def_port = def.port;
    let def_opts = def.opts;
    let def_bastion = def.bastion;
    let def_jump = def.jump;
    let use_system_ssh_config = def.use_system_ssh_config;
    let def_tunnels = def.tunnels;
    let namespace = def.namespace;
    let vars = def.vars;
    let def_control_master = def.control_master;
    let def_agent_forwarding = def.agent_forwarding;
    let def_control_path = def.control_path;
    let def_control_persist = def.control_persist;
    let def_pre_connect_hook = def.pre_connect_hook;
    let def_post_disconnect_hook = def.post_disconnect_hook;
    let def_hook_timeout_secs = def.hook_timeout_secs;
    let user = interpolate(s.user.as_deref().or(def_user).unwrap_or("root"), vars);
    let port = s.ssh_port.or(def_port).unwrap_or(22);
    let key = interpolate(
        s.ssh_key.as_deref().or(def_key).unwrap_or("~/.ssh/id_rsa"),
        vars,
    );
    let cert = s
        .ssh_cert
        .as_deref()
        .or(def_cert)
        .map(|c| shellexpand::tilde(c).into_owned())
        .unwrap_or_default();
    let agent_sock = s
        .ssh_agent_sock
        .as_deref()
        .or(def_agent_sock)
        .map(|c| shellexpand::tilde(c).into_owned())
        .unwrap_or_default();

    let opts = if let Some(o) = &s.ssh_options {
        o.clone()
    } else {
        def_opts.cloned().unwrap_or_default()
    };

    let probe_filesystems =
        extend_filesystems(def.fs.as_ref(), s.probe_filesystems.as_ref()).unwrap_or_default();

    let tunnels = s
        .tunnels
        .as_ref()
        .or(def_tunnels)
        .cloned()
        .unwrap_or_default();

    let final_bastion = merge_bastion(def_bastion, &s.wallix);
    let final_jump = merge_jump(def_jump, &s.jump);

    let mode = s.mode.or(def_mode).unwrap_or(ConnectionMode::Direct);

    let bastion_template = final_bastion
        .as_ref()
        .and_then(|b| b.template.clone())
        .unwrap_or_else(|| "{target_user}@%n:SSH:{bastion_user}".to_string());

    let jump_host = final_jump.as_ref().map(|jumps| {
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

    // Priorité déterministe pour le groupe Wallix:
    // 1) server.wallix.group
    // 2) server.wallix_group (legacy)
    // 3) héritage déjà fusionné via final_bastion.group (env/group/defaults)
    let resolved_wallix_group = s
        .wallix
        .as_ref()
        .and_then(|b| b.group.as_deref())
        .or(s.wallix_group.as_deref())
        .or(final_bastion.as_ref().and_then(|b| b.group.as_deref()))
        .map(str::trim)
        .filter(|g| !g.is_empty())
        .map(ToOwned::to_owned);

    Ok(ResolvedServer {
        namespace: namespace.to_string(),
        group_name: group.to_string(),
        env_name: env.to_string(),
        name: interpolate(&s.name, vars),
        host: interpolate(&s.host, vars),
        user,
        port,
        ssh_key: key,
        ssh_cert: cert,
        ssh_agent_sock: agent_sock,
        ssh_options: opts,
        default_mode: mode,
        jump_host,
        bastion_host: final_bastion.as_ref().and_then(|b| b.host.clone()),
        bastion_user: final_bastion.as_ref().and_then(|b| b.user.clone()),
        bastion_template,
        use_system_ssh_config,
        probe_filesystems,
        tunnels,
        tags: extend_tags(Some(&def.tags), s.tags.as_ref()),
        control_master: def_control_master,
        agent_forwarding: def_agent_forwarding,
        control_path: if def_control_master {
            shellexpand::tilde(def_control_path).into_owned()
        } else {
            String::new()
        },
        control_persist: def_control_persist.to_string(),
        pre_connect_hook: s
            .pre_connect_hook
            .as_deref()
            .or(def_pre_connect_hook)
            .map(|h| shellexpand::tilde(h).into_owned()),
        post_disconnect_hook: s
            .post_disconnect_hook
            .as_deref()
            .or(def_post_disconnect_hook)
            .map(|h| shellexpand::tilde(h).into_owned()),
        hook_timeout_secs: def_hook_timeout_secs,
        notes: s.notes.clone().unwrap_or_default(),
        wallix_group: resolved_wallix_group,
        wallix_account: final_bastion
            .as_ref()
            .and_then(|b| b.account.clone())
            .unwrap_or_else(|| "default".to_string()),
        wallix_protocol: final_bastion
            .as_ref()
            .and_then(|b| b.protocol.clone())
            .unwrap_or_else(|| "SSH".to_string()),
        wallix_auto_select: final_bastion
            .as_ref()
            .and_then(|b| b.auto_select)
            .unwrap_or(true),
        wallix_fail_if_menu_match_error: final_bastion
            .as_ref()
            .and_then(|b| b.fail_if_menu_match_error)
            .unwrap_or(true),
        wallix_selection_timeout_secs: final_bastion
            .as_ref()
            .and_then(|b| b.selection_timeout_secs)
            .unwrap_or(8),
        wallix_direct: final_bastion
            .as_ref()
            .and_then(|b| b.direct)
            .unwrap_or(false),
        wallix_authorization: final_bastion.as_ref().and_then(|b| b.authorization.clone()),
        wallix_header_columns: final_bastion
            .as_ref()
            .and_then(|b| b.header_columns.clone())
            .unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Tests interpolate / undefined_vars ───────────────────────────────────

    #[test]
    fn test_interpolate_replaces_known_vars() {
        let vars = HashMap::from([
            ("host".to_string(), "bastion.prod.example.com".to_string()),
            ("env".to_string(), "prod".to_string()),
        ]);
        assert_eq!(interpolate("{{ host }}", &vars), "bastion.prod.example.com");
        assert_eq!(interpolate("{{ env }}-server", &vars), "prod-server");
        assert_eq!(
            interpolate("{{ env }}.{{ host }}", &vars),
            "prod.bastion.prod.example.com"
        );
    }

    #[test]
    fn test_interpolate_leaves_undefined_vars() {
        let vars = HashMap::new();
        assert_eq!(interpolate("{{ unknown }}", &vars), "{{ unknown }}");
    }

    #[test]
    fn test_interpolate_no_placeholder() {
        let vars = HashMap::from([("x".to_string(), "y".to_string())]);
        assert_eq!(interpolate("plain-host", &vars), "plain-host");
    }

    #[test]
    fn test_undefined_vars_finds_missing() {
        let vars = HashMap::from([("a".to_string(), "1".to_string())]);
        let result = undefined_vars("{{ a }} and {{ b }}", &vars);
        assert_eq!(result, vec!["b".to_string()]);
    }

    #[test]
    fn test_undefined_vars_empty_when_all_defined() {
        let vars = HashMap::from([("x".to_string(), "v".to_string())]);
        assert!(undefined_vars("{{ x }}", &vars).is_empty());
    }

    #[test]
    fn test_resolve_applies_interpolation() {
        let vars = HashMap::from([("jump".to_string(), "bastion.example.com".to_string())]);
        let config = Config {
            defaults: None,
            groups: vec![ConfigEntry::Group(Group {
                name: "G".to_string(),
                user: None,
                ssh_key: None,
                mode: None,
                ssh_port: None,
                ssh_options: None,
                wallix: None,
                wallix_group: None,
                jump: None,
                probe_filesystems: None,
                environments: None,
                tunnels: None,
                tags: None,
                servers: Some(vec![Server {
                    name: "jump-srv".to_string(),
                    host: "{{ jump }}".to_string(),
                    user: None,
                    ssh_key: None,
                    ssh_port: None,
                    ssh_options: None,
                    mode: None,
                    wallix: None,
                    jump: None,
                    probe_filesystems: None,
                    tunnels: None,
                    tags: None,
                    ..Default::default()
                }]),
            })],
            includes: vec![],
            vars,
        };

        let resolved = config.resolve().unwrap();
        assert_eq!(resolved[0].host, "bastion.example.com");
        assert_eq!(resolved[0].name, "jump-srv");
    }

    #[test]
    fn test_merge_bastion() {
        let parent = Some(BastionConfig {
            host: Some("parent_host".to_string()),
            user: Some("parent_user".to_string()),
            group: Some("parent-group".to_string()),
            template: Some("parent_tmpl".to_string()),
            account: None,
            protocol: None,
            auto_select: None,
            fail_if_menu_match_error: None,
            selection_timeout_secs: None,
            direct: None,
            authorization: None,
            header_columns: None,
        });
        let child = BastionConfig {
            host: None,
            user: Some("child_user".to_string()),
            group: None,
            template: None,
            account: None,
            protocol: None,
            auto_select: None,
            fail_if_menu_match_error: None,
            selection_timeout_secs: None,
            direct: None,
            authorization: None,
            header_columns: None,
        };

        let merged = merge_bastion(&parent, &Some(child)).unwrap();
        // Child user overrides parent
        assert_eq!(merged.user, Some("child_user".to_string()));
        // Parent host is inherited
        assert_eq!(merged.host, Some("parent_host".to_string()));
        // Parent template is inherited
        assert_eq!(merged.template, Some("parent_tmpl".to_string()));
        // Parent group is inherited
        assert_eq!(merged.group, Some("parent-group".to_string()));
    }

    #[test]
    fn test_resolve_inherits_wallix_group_from_defaults_wallix() {
        let config = Config {
            defaults: Some(Defaults {
                mode: Some(ConnectionMode::Wallix),
                wallix: Some(BastionConfig {
                    host: Some("bastion.example.test".to_string()),
                    user: Some("demo_user".to_string()),
                    group: Some("dev-admins".to_string()),
                    template: None,
                    account: None,
                    protocol: None,
                    auto_select: None,
                    fail_if_menu_match_error: None,
                    selection_timeout_secs: None,
                    direct: None,
                    authorization: None,
                    header_columns: None,
                }),
                ..Default::default()
            }),
            groups: vec![ConfigEntry::Group(Group {
                name: "ALPHA-BD".to_string(),
                user: None,
                ssh_key: None,
                mode: None,
                ssh_port: None,
                ssh_options: None,
                wallix: None,
                wallix_group: None,
                jump: None,
                environments: None,
                servers: Some(vec![Server {
                    name: "app-alpha".to_string(),
                    host: "APP-ALPHA-BD".to_string(),
                    user: None,
                    ssh_key: None,
                    ssh_cert: None,
                    ssh_port: None,
                    ssh_options: None,
                    mode: None,
                    wallix: None,
                    wallix_group: None,
                    jump: None,
                    probe_filesystems: None,
                    tunnels: None,
                    tags: None,
                    pre_connect_hook: None,
                    post_disconnect_hook: None,
                    notes: None,
                    ssh_agent_sock: None,
                }]),
                probe_filesystems: None,
                tunnels: None,
                tags: None,
            })],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        assert_eq!(resolved[0].wallix_group.as_deref(), Some("dev-admins"));
    }

    #[test]
    fn test_resolve_wallix_group_server_override_wins_over_global() {
        let config = Config {
            defaults: Some(Defaults {
                wallix: Some(BastionConfig {
                    group: Some("global-admins".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            groups: vec![ConfigEntry::Server(Server {
                name: "srv".to_string(),
                host: "srv.example.test".to_string(),
                wallix: Some(BastionConfig {
                    group: Some("conn-admins".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            })],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        assert_eq!(resolved[0].wallix_group.as_deref(), Some("conn-admins"));
    }

    #[test]
    fn test_resolve_wallix_group_env_override_wins_over_global() {
        let config = Config {
            defaults: Some(Defaults {
                wallix: Some(BastionConfig {
                    group: Some("global-admins".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            groups: vec![ConfigEntry::Group(Group {
                name: "G".to_string(),
                user: None,
                ssh_key: None,
                mode: None,
                ssh_port: None,
                ssh_options: None,
                wallix: None,
                wallix_group: None,
                jump: None,
                environments: Some(vec![Environment {
                    name: "PR-OND".to_string(),
                    user: None,
                    ssh_key: None,
                    mode: None,
                    ssh_port: None,
                    ssh_options: None,
                    wallix: Some(BastionConfig {
                        group: Some("env-admins".to_string()),
                        ..Default::default()
                    }),
                    wallix_group: None,
                    jump: None,
                    servers: vec![Server {
                        name: "db07".to_string(),
                        host: "db07.internal.example".to_string(),
                        ..Default::default()
                    }],
                    probe_filesystems: None,
                    tunnels: None,
                    tags: None,
                }]),
                servers: None,
                probe_filesystems: None,
                tunnels: None,
                tags: None,
            })],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        assert_eq!(resolved[0].wallix_group.as_deref(), Some("env-admins"));
    }

    #[test]
    fn test_resolve_wallix_group_none_when_missing_everywhere() {
        let config = Config {
            defaults: Some(Defaults::default()),
            groups: vec![ConfigEntry::Server(Server {
                name: "srv".to_string(),
                host: "srv.example.test".to_string(),
                ..Default::default()
            })],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        assert_eq!(resolved[0].wallix_group, None);
    }

    #[test]
    fn test_sorting_mixed() {
        let mut config = Config {
            defaults: None,
            groups: vec![
                ConfigEntry::Group(Group {
                    name: "Zeus".to_string(),
                    user: None,
                    ssh_key: None,
                    mode: None,
                    ssh_port: None,
                    ssh_options: None,
                    wallix: None,
                    wallix_group: None,
                    jump: None,
                    environments: None,
                    servers: None,
                    probe_filesystems: None,
                    tunnels: None,
                    tags: None,
                }),
                ConfigEntry::Server(Server {
                    name: "Alpha".to_string(),
                    host: "198.51.100.1".to_string(),
                    user: None,
                    ssh_key: None,
                    ssh_port: None,
                    ssh_options: None,
                    mode: None,
                    wallix: None,
                    jump: None,
                    probe_filesystems: None,
                    tunnels: None,
                    tags: None,
                    ..Default::default()
                }),
                ConfigEntry::Group(Group {
                    name: "Beta".to_string(),
                    user: None,
                    ssh_key: None,
                    mode: None,
                    ssh_port: None,
                    ssh_options: None,
                    wallix: None,
                    wallix_group: None,
                    jump: None,
                    environments: None,
                    servers: None,
                    probe_filesystems: None,
                    tunnels: None,
                    tags: None,
                }),
            ],
            includes: vec![],
            vars: Default::default(),
        };

        config.sort();

        // Check order: Alpha, Beta, Zeus
        match &config.groups[0] {
            ConfigEntry::Server(s) => assert_eq!(s.name, "Alpha"),
            _ => panic!("Expected Alpha first"),
        }
        match &config.groups[1] {
            ConfigEntry::Group(g) => assert_eq!(g.name, "Beta"),
            _ => panic!("Expected Beta second"),
        }
        match &config.groups[2] {
            ConfigEntry::Group(g) => assert_eq!(g.name, "Zeus"),
            _ => panic!("Expected Zeus third"),
        }
    }

    #[test]
    fn test_resolve_inheritance_chain() {
        let config = Config {
            defaults: Some(Defaults {
                user: Some("default_user".to_string()),
                ssh_port: Some(2222),
                ..Default::default()
            }),
            groups: vec![ConfigEntry::Group(Group {
                name: "G1".to_string(),
                user: Some("group_user".to_string()), // Override default
                ssh_key: None,
                mode: None,
                ssh_port: None, // Inherits 2222
                ssh_options: None,
                wallix: None,
                wallix_group: None,
                jump: None,
                probe_filesystems: None,
                tags: None,
                environments: Some(vec![Environment {
                    name: "Env1".to_string(),
                    user: None, // Inherits "group_user"
                    ssh_key: None,
                    mode: None,
                    ssh_port: None, // Inherits 2222
                    ssh_options: None,
                    wallix: None,
                    wallix_group: None,
                    jump: None,
                    probe_filesystems: None,
                    tunnels: None,
                    tags: None,
                    servers: vec![Server {
                        name: "S1".to_string(),
                        host: "203.0.113.1".to_string(),
                        user: None, // Inherits "group_user"
                        ssh_key: None,
                        ssh_port: Some(8080), // Override 2222
                        ssh_options: None,
                        mode: None,
                        wallix: None,
                        jump: None,
                        probe_filesystems: None,
                        tunnels: None,
                        tags: None,
                        ..Default::default()
                    }],
                }]),
                servers: None,
                tunnels: None,
            })],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        let s1 = &resolved[0];

        assert_eq!(s1.name, "S1");
        assert_eq!(s1.user, "group_user");
        assert_eq!(s1.port, 8080);
    }

    #[test]
    fn test_probe_filesystems_inheritance() {
        let config = Config {
            defaults: Some(Defaults {
                probe_filesystems: Some(vec!["/data".to_string(), "/var/log".to_string()]),
                ..Default::default()
            }),
            groups: vec![ConfigEntry::Group(Group {
                name: "G".to_string(),
                user: None,
                ssh_key: None,
                mode: None,
                ssh_port: None,
                ssh_options: None,
                wallix: None,
                wallix_group: None,
                jump: None,
                probe_filesystems: None, // hérite des defaults
                environments: None,
                tunnels: None,
                tags: None,
                servers: Some(vec![
                    Server {
                        name: "inherits".to_string(),
                        host: "203.0.113.4".to_string(),
                        user: None,
                        ssh_key: None,
                        ssh_port: None,
                        ssh_options: None,
                        mode: None,
                        wallix: None,
                        jump: None,
                        probe_filesystems: None, // hérite du groupe → defaults
                        tunnels: None,
                        tags: None,
                        ..Default::default()
                    },
                    Server {
                        name: "extends".to_string(),
                        host: "203.0.113.5".to_string(),
                        user: None,
                        ssh_key: None,
                        ssh_port: None,
                        ssh_options: None,
                        mode: None,
                        wallix: None,
                        jump: None,
                        probe_filesystems: Some(vec!["/mnt/nas".to_string()]), // s'ajoute aux defaults
                        tunnels: None,
                        tags: None,
                        ..Default::default()
                    },
                ]),
            })],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();

        let inherits = resolved.iter().find(|s| s.name == "inherits").unwrap();
        assert_eq!(
            inherits.probe_filesystems,
            vec!["/data".to_string(), "/var/log".to_string()]
        );

        // Le serveur ajoute /mnt/nas aux defaults — il ne les remplace PAS
        let extends = resolved.iter().find(|s| s.name == "extends").unwrap();
        assert_eq!(
            extends.probe_filesystems,
            vec![
                "/data".to_string(),
                "/var/log".to_string(),
                "/mnt/nas".to_string()
            ]
        );
    }

    #[test]
    fn test_probe_filesystems_group_extends_defaults() {
        let config = Config {
            defaults: Some(Defaults {
                probe_filesystems: Some(vec!["/pg_backup".to_string(), "/pg_xlogs".to_string()]),
                ..Default::default()
            }),
            groups: vec![ConfigEntry::Group(Group {
                name: "ALPHA".to_string(),
                user: None,
                ssh_key: None,
                mode: None,
                ssh_port: None,
                ssh_options: None,
                wallix: None,
                wallix_group: None,
                jump: None,
                probe_filesystems: Some(vec!["/kafka_data".to_string()]), // s'ajoute
                environments: None,
                tunnels: None,
                tags: None,
                servers: Some(vec![Server {
                    name: "kafka01".to_string(),
                    host: "198.51.100.1".to_string(),
                    user: None,
                    ssh_key: None,
                    ssh_port: None,
                    ssh_options: None,
                    mode: None,
                    wallix: None,
                    jump: None,
                    probe_filesystems: None,
                    tunnels: None,
                    tags: None,
                    ..Default::default()
                }]),
            })],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        let kafka = resolved.iter().find(|s| s.name == "kafka01").unwrap();

        // Le groupe ajoute /kafka_data aux defaults — PG filesystems toujours présents
        assert_eq!(
            kafka.probe_filesystems,
            vec![
                "/pg_backup".to_string(),
                "/pg_xlogs".to_string(),
                "/kafka_data".to_string()
            ]
        );
    }

    // ─── Tests includes / namespaces ─────────────────────────────────────────

    fn write_temp_yaml(content: &str) -> tempfile::NamedTempFile {
        use std::io::Write;
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn test_includes_basic() {
        let sub_yaml = r#"
defaults:
  user: "sub_user"
groups:
  - name: NS_Group
    servers:
      - name: ns_srv
        host: "198.51.100.1"
"#;
        let sub_file = write_temp_yaml(sub_yaml);

        let main_yaml = format!(
            r#"
defaults:
  user: "main_user"
includes:
  - label: "CES"
    path: "{}"
groups:
  - name: Main_Group
    servers:
      - name: main_srv
        host: "198.51.100.1"
"#,
            sub_file.path().to_string_lossy()
        );
        let main_file = write_temp_yaml(&main_yaml);

        let (config, warnings, _val) =
            Config::load_merged(main_file.path(), &mut std::collections::HashSet::new()).unwrap();
        assert!(warnings.is_empty(), "Expected no warnings: {:?}", warnings);

        let resolved = config.resolve().unwrap();

        // main_srv has empty namespace
        let main_srv = resolved.iter().find(|s| s.name == "main_srv").unwrap();
        assert_eq!(main_srv.namespace, "");
        assert_eq!(main_srv.user, "main_user");

        // ns_srv has namespace "CES" and uses sub-config defaults
        let ns_srv = resolved.iter().find(|s| s.name == "ns_srv").unwrap();
        assert_eq!(ns_srv.namespace, "CES");
        assert_eq!(ns_srv.user, "sub_user");

        // Config tree should contain a Namespace entry
        let has_namespace = config.groups.iter().any(|e| {
            if let ConfigEntry::Namespace(ns) = e {
                ns.label == "CES"
            } else {
                false
            }
        });
        assert!(has_namespace, "Expected Namespace(CES) in config.groups");
    }

    #[test]
    fn test_includes_defaults_isolation() {
        let sub_yaml = r#"
defaults:
  user: "sub_user"
  ssh_port: 9999
groups:
  - name: Sub
    servers:
      - name: sub_srv
        host: "203.0.113.4"
"#;
        let sub_file = write_temp_yaml(sub_yaml);

        let main_yaml = format!(
            r#"
defaults:
  user: "main_user"
  ssh_port: 22
includes:
  - label: "SUB"
    path: "{}"
groups:
  - name: Main
    servers:
      - name: main_srv
        host: "203.0.113.8"
"#,
            sub_file.path().to_string_lossy()
        );
        let main_file = write_temp_yaml(&main_yaml);

        let (config, warnings, _val) =
            Config::load_merged(main_file.path(), &mut std::collections::HashSet::new()).unwrap();
        assert!(warnings.is_empty());

        let resolved = config.resolve().unwrap();

        let main_srv = resolved.iter().find(|s| s.name == "main_srv").unwrap();
        // Main defaults apply to main_srv
        assert_eq!(main_srv.user, "main_user");
        assert_eq!(main_srv.port, 22);

        let sub_srv = resolved.iter().find(|s| s.name == "sub_srv").unwrap();
        // Sub defaults apply only to sub_srv, not leaked from main
        assert_eq!(sub_srv.user, "sub_user");
        assert_eq!(sub_srv.port, 9999);
    }

    #[test]
    fn test_includes_missing_file() {
        let main_yaml = r#"
defaults:
  user: "admin"
includes:
  - label: "MISSING"
    path: "/tmp/susshi_nonexistent_test_file_xyz.yml"
groups:
  - name: Main
    servers:
      - name: ok_srv
        host: "203.0.113.4"
"#;
        let main_file = write_temp_yaml(main_yaml);

        let (config, warnings, _val) =
            Config::load_merged(main_file.path(), &mut std::collections::HashSet::new()).unwrap();

        // Un avertissement LoadError doit être émis
        assert_eq!(warnings.len(), 1);
        if let IncludeWarning::LoadError { label, .. } = &warnings[0] {
            assert_eq!(label, "MISSING");
        } else {
            panic!("Expected LoadError warning, got {:?}", warnings[0]);
        }

        // Les groupes du fichier principal sont toujours résolus
        let resolved = config.resolve().unwrap();
        assert!(resolved.iter().any(|s| s.name == "ok_srv"));
    }

    #[test]
    fn test_includes_nested_recursive() {
        // Fichier inclus qui contient lui-même un `includes:` — résolution récursive v0.8
        let leaf_yaml = r#"
groups:
  - name: Leaf
    servers:
      - name: leaf_srv
        host: "203.0.113.9"
"#;
        let leaf_file = write_temp_yaml(leaf_yaml);

        let sub_yaml = format!(
            r#"
includes:
  - label: "LEAF"
    path: "{}"
groups:
  - name: Sub
    servers:
      - name: sub_srv
        host: "203.0.113.18"
"#,
            leaf_file.path().to_string_lossy()
        );
        let sub_file = write_temp_yaml(&sub_yaml);

        let main_yaml = format!(
            r#"
includes:
  - label: "SUB"
    path: "{}"
groups: []
"#,
            sub_file.path().to_string_lossy()
        );
        let main_file = write_temp_yaml(&main_yaml);

        let (config, warnings, _val) =
            Config::load_merged(main_file.path(), &mut std::collections::HashSet::new()).unwrap();

        // Aucun avertissement : les includes imbriqués sont désormais résolus récursivement
        assert!(
            warnings.is_empty(),
            "Expected no warnings, got: {:?}",
            warnings
        );

        // Les deux namespaces aplatis sont présents
        let labels: Vec<&str> = config
            .groups
            .iter()
            .filter_map(|e| {
                if let ConfigEntry::Namespace(ns) = e {
                    Some(ns.label.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert!(labels.contains(&"SUB"), "Missing SUB, got {:?}", labels);
        assert!(
            labels.contains(&"SUB / LEAF"),
            "Missing 'SUB / LEAF', got {:?}",
            labels
        );

        let resolved = config.resolve().unwrap();
        assert!(
            resolved
                .iter()
                .any(|s| s.name == "sub_srv" && s.namespace == "SUB")
        );
        assert!(
            resolved
                .iter()
                .any(|s| s.name == "leaf_srv" && s.namespace == "SUB / LEAF")
        );
    }

    #[test]
    fn test_includes_merge_defaults() {
        let sub_yaml = r#"
defaults:
  user: "sub_user"
groups:
  - name: Sub
    servers:
      - name: sub_srv
        host: "203.0.113.4"
"#;
        let sub_file = write_temp_yaml(sub_yaml);

        let main_yaml = format!(
            r#"
defaults:
  user: "main_user"
  ssh_port: 2222
includes:
  - label: "SUB"
    path: "{}"
    merge_defaults: true
groups: []
"#,
            sub_file.path().to_string_lossy()
        );
        let main_file = write_temp_yaml(&main_yaml);

        let (config, _warnings, _val) =
            Config::load_merged(main_file.path(), &mut std::collections::HashSet::new()).unwrap();
        let resolved = config.resolve().unwrap();

        let sub_srv = resolved.iter().find(|s| s.name == "sub_srv").unwrap();
        // Sub defaults override main defaults for user
        assert_eq!(sub_srv.user, "sub_user");
        // Main port is inherited since sub didn't specify ssh_port
        assert_eq!(sub_srv.port, 2222);
    }

    /// Les defaults du fichier principal sont automatiquement hérités par les
    /// namespaces inclus, même sans `merge_defaults: true`.
    #[test]
    fn test_includes_inherit_main_defaults_automatically() {
        let sub_yaml = r#"
groups:
  - name: SubGroup
    servers:
      - name: sub_srv
        host: "2.3.4.5"
"#;
        let sub_file = write_temp_yaml(sub_yaml);

        let main_yaml = format!(
            r#"
defaults:
  user: "main_user"
  ssh_port: 2222
  jump:
    - host: "jump.example.com"
      user: "juser"
includes:
  - label: "SUB"
    path: "{}"
groups: []
"#,
            sub_file.path().to_string_lossy()
        );
        let main_file = write_temp_yaml(&main_yaml);

        let (config, _warnings, _val) =
            Config::load_merged(main_file.path(), &mut std::collections::HashSet::new()).unwrap();
        let resolved = config.resolve().unwrap();

        let sub_srv = resolved.iter().find(|s| s.name == "sub_srv").unwrap();
        // Les defaults du principal doivent être hérités sans merge_defaults: true
        assert_eq!(sub_srv.user, "main_user");
        assert_eq!(sub_srv.port, 2222);
        assert_eq!(sub_srv.jump_host.as_deref(), Some("juser@jump.example.com"));
    }

    #[test]
    fn test_includes_circular() {
        let file_a = tempfile::NamedTempFile::new().unwrap();
        let file_b = tempfile::NamedTempFile::new().unwrap();

        let yaml_a = format!(
            r#"
includes:
  - label: "B"
    path: "{}"
groups:
  - name: GroupA
    servers: [{{ name: srv_a, host: "198.51.100.1" }}]
"#,
            file_b.path().display()
        );
        let yaml_b = format!(
            r#"
includes:
  - label: "A"
    path: "{}"
groups:
  - name: GroupB
    servers: [{{ name: srv_b, host: "198.51.100.2" }}]
"#,
            file_a.path().display()
        );
        std::fs::write(file_a.path(), yaml_a.as_bytes()).unwrap();
        std::fs::write(file_b.path(), yaml_b.as_bytes()).unwrap();

        let (config, warnings, _val) =
            Config::load_merged(file_a.path(), &mut std::collections::HashSet::new()).unwrap();

        let has_circular = warnings
            .iter()
            .any(|w| matches!(w, IncludeWarning::Circular { .. }));
        assert!(
            has_circular,
            "Expected Circular warning, got: {:?}",
            warnings
        );

        let resolved = config.resolve().unwrap();
        assert!(
            resolved
                .iter()
                .any(|s| s.name == "srv_a" || s.name == "srv_b"),
            "Should resolve at least one server"
        );
    }

    #[test]
    fn test_validation_unknown_field() {
        let yaml = r#"
defaults:
  user: "admin"
  typo_field: "oops"
groups: []
"#;
        let warnings = validate_yaml(yaml, "test.yml");
        assert!(
            warnings.iter().any(|w| w.field == "typo_field"),
            "Expected ValidationWarning for typo_field, got: {:?}",
            warnings
        );
    }

    #[test]
    fn test_validation_unknown_server_field() {
        let yaml = r#"
groups:
  - name: G
    servers:
      - name: srv
        host: "203.0.113.4"
        missspelled_user: "admin"
"#;
        let warnings = validate_yaml(yaml, "test.yml");
        assert!(
            warnings.iter().any(|w| w.field == "missspelled_user"),
            "Expected ValidationWarning for missspelled_user, got: {:?}",
            warnings
        );
    }

    #[test]
    fn test_namespace_server_has_namespace_field() {
        let sub_yaml = r#"
groups:
  - name: NS_G
    servers:
      - name: ns_srv
        host: "198.51.100.101"
        user: "ns_user"
"#;
        let sub_file = write_temp_yaml(sub_yaml);

        let main_yaml = format!(
            r#"
includes:
  - label: "CRT"
    path: "{}"
groups: []
"#,
            sub_file.path().to_string_lossy()
        );
        let main_file = write_temp_yaml(&main_yaml);

        let (config, _, _) =
            Config::load_merged(main_file.path(), &mut std::collections::HashSet::new()).unwrap();
        let resolved = config.resolve().unwrap();

        let ns_srv = resolved.iter().find(|s| s.name == "ns_srv").unwrap();
        assert_eq!(ns_srv.namespace, "CRT");
        assert_eq!(ns_srv.group_name, "NS_G");
    }

    // ─── Tests keep_open ─────────────────────────────────────────────────────

    #[test]
    fn test_keep_open_absent_defaults_to_none() {
        let yaml = r#"
groups: []
"#;
        let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
        assert!(config.defaults.is_none() || config.defaults.unwrap().keep_open.is_none());
    }

    #[test]
    fn test_keep_open_true_parsed() {
        let yaml = r#"
defaults:
  keep_open: true
groups: []
"#;
        let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
        let keep_open = config
            .defaults
            .as_ref()
            .and_then(|d| d.keep_open)
            .unwrap_or(false);
        assert!(keep_open);
    }

    #[test]
    fn test_keep_open_false_parsed() {
        let yaml = r#"
defaults:
  keep_open: false
groups: []
"#;
        let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
        let keep_open = config
            .defaults
            .as_ref()
            .and_then(|d| d.keep_open)
            .unwrap_or(true); // on passe true pour détecter si false est bien parsé
        assert!(!keep_open);
    }

    #[test]
    fn test_keep_open_no_validation_warning() {
        let yaml = r#"
defaults:
  keep_open: true
groups: []
"#;
        let warnings = validate_yaml(yaml, "test.yaml");
        assert!(
            warnings.is_empty(),
            "keep_open should not produce a ValidationWarning, got: {:?}",
            warnings
        );
    }

    // ─── Tests tag inheritance ────────────────────────────────────────────────

    fn make_server(name: &str, host: &str, tags: Option<Vec<&str>>) -> Server {
        Server {
            name: name.to_string(),
            host: host.to_string(),
            tags: tags.map(|v| v.into_iter().map(str::to_owned).collect()),
            ..Default::default()
        }
    }

    fn make_group(name: &str, servers: Vec<Server>) -> Group {
        Group {
            name: name.to_string(),
            user: None,
            ssh_key: None,
            mode: None,
            ssh_port: None,
            ssh_options: None,
            wallix: None,
            wallix_group: None,
            jump: None,
            probe_filesystems: None,
            tunnels: None,
            tags: None,
            environments: None,
            servers: Some(servers),
        }
    }

    fn make_group_with_env(name: &str, envs: Vec<Environment>) -> Group {
        Group {
            name: name.to_string(),
            user: None,
            ssh_key: None,
            mode: None,
            ssh_port: None,
            ssh_options: None,
            wallix: None,
            wallix_group: None,
            jump: None,
            probe_filesystems: None,
            tunnels: None,
            tags: None,
            environments: Some(envs),
            servers: None,
        }
    }

    fn make_env(name: &str, servers: Vec<Server>) -> Environment {
        Environment {
            name: name.to_string(),
            user: None,
            ssh_key: None,
            mode: None,
            ssh_port: None,
            ssh_options: None,
            wallix: None,
            wallix_group: None,
            jump: None,
            probe_filesystems: None,
            tunnels: None,
            tags: None,
            servers,
        }
    }

    #[test]
    fn test_tags_union_across_all_levels() {
        let mut env = make_env(
            "E",
            vec![make_server("srv", "10.0.0.1", Some(vec!["srv-tag"]))],
        );
        env.tags = Some(vec!["env-tag".to_string()]);
        let mut group = make_group_with_env("G", vec![env]);
        group.tags = Some(vec!["group-tag".to_string()]);
        let config = Config {
            defaults: Some(Defaults {
                tags: Some(vec!["global".to_string()]),
                ..Default::default()
            }),
            groups: vec![ConfigEntry::Group(group)],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        let tags = &resolved[0].tags;
        assert!(tags.contains(&"global".to_string()), "missing defaults tag");
        assert!(tags.contains(&"group-tag".to_string()), "missing group tag");
        assert!(tags.contains(&"env-tag".to_string()), "missing env tag");
        assert!(tags.contains(&"srv-tag".to_string()), "missing server tag");
        assert_eq!(tags.len(), 4);
    }

    #[test]
    fn test_tags_deduplication_across_levels() {
        let env = make_env(
            "E",
            vec![make_server("srv", "10.0.0.1", Some(vec!["shared"]))],
        );
        let mut group = make_group_with_env("G", vec![env]);
        group.tags = Some(vec!["shared".to_string()]);
        let config = Config {
            defaults: Some(Defaults {
                tags: Some(vec!["shared".to_string(), "global".to_string()]),
                ..Default::default()
            }),
            groups: vec![ConfigEntry::Group(group)],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        let tags = &resolved[0].tags;
        assert_eq!(
            tags.iter().filter(|t| t.as_str() == "shared").count(),
            1,
            "shared tag should appear only once, got: {:?}",
            tags
        );
        assert_eq!(tags.len(), 2);
    }

    #[test]
    fn test_tags_group_level_server_inherits_defaults_and_group() {
        let mut group = make_group("G", vec![make_server("srv", "10.0.0.1", None)]);
        group.tags = Some(vec!["group-tag".to_string()]);
        let config = Config {
            defaults: Some(Defaults {
                tags: Some(vec!["defaults-tag".to_string()]),
                ..Default::default()
            }),
            groups: vec![ConfigEntry::Group(group)],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        let tags = &resolved[0].tags;
        assert!(tags.contains(&"defaults-tag".to_string()));
        assert!(tags.contains(&"group-tag".to_string()));
        assert_eq!(tags.len(), 2);
    }

    #[test]
    fn test_tags_top_level_server_inherits_defaults() {
        let config = Config {
            defaults: Some(Defaults {
                tags: Some(vec!["top-tag".to_string()]),
                ..Default::default()
            }),
            groups: vec![ConfigEntry::Server(Server {
                name: "bare-srv".to_string(),
                host: "10.0.0.1".to_string(),
                ..Default::default()
            })],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        assert!(resolved[0].tags.contains(&"top-tag".to_string()));
    }

    // ─── Tests tunnel REPLACE semantics ──────────────────────────────────────

    fn tunnel(local: u16, remote: u16) -> TunnelConfig {
        TunnelConfig {
            local_port: local,
            remote_host: "127.0.0.1".to_string(),
            remote_port: remote,
            label: String::new(),
        }
    }

    #[test]
    fn test_tunnels_defaults_inherited_when_no_child_defines_them() {
        let config = Config {
            defaults: Some(Defaults {
                tunnels: Some(vec![tunnel(5432, 5432)]),
                ..Default::default()
            }),
            groups: vec![ConfigEntry::Group(make_group_with_env(
                "G",
                vec![make_env("E", vec![make_server("srv", "10.0.0.1", None)])],
            ))],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        assert_eq!(resolved[0].tunnels, vec![tunnel(5432, 5432)]);
    }

    #[test]
    fn test_tunnels_group_replaces_defaults() {
        let mut group = make_group_with_env(
            "G",
            vec![make_env("E", vec![make_server("srv", "10.0.0.1", None)])],
        );
        group.tunnels = Some(vec![tunnel(6379, 6379)]);
        let config = Config {
            defaults: Some(Defaults {
                tunnels: Some(vec![tunnel(5432, 5432)]),
                ..Default::default()
            }),
            groups: vec![ConfigEntry::Group(group)],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        assert_eq!(
            resolved[0].tunnels,
            vec![tunnel(6379, 6379)],
            "group tunnels should fully replace defaults tunnels"
        );
    }

    #[test]
    fn test_tunnels_server_replaces_env() {
        let mut env = make_env(
            "E",
            vec![Server {
                name: "srv".to_string(),
                host: "10.0.0.1".to_string(),
                tunnels: Some(vec![tunnel(8080, 80)]),
                ..Default::default()
            }],
        );
        env.tunnels = Some(vec![tunnel(5432, 5432)]);
        let config = Config {
            defaults: None,
            groups: vec![ConfigEntry::Group(make_group_with_env("G", vec![env]))],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        assert_eq!(
            resolved[0].tunnels,
            vec![tunnel(8080, 80)],
            "server tunnels should fully replace env tunnels"
        );
    }

    // ─── Tests group-level servers (no environment) ───────────────────────────

    #[test]
    fn test_group_server_without_environment() {
        let config = Config {
            defaults: Some(Defaults {
                user: Some("default-user".to_string()),
                ssh_port: Some(2222),
                ..Default::default()
            }),
            groups: vec![ConfigEntry::Group(make_group(
                "Flat",
                vec![make_server("srv", "10.0.0.1", None)],
            ))],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        assert_eq!(resolved[0].user, "default-user");
        assert_eq!(resolved[0].port, 2222);
        assert_eq!(resolved[0].group_name, "Flat");
        assert_eq!(resolved[0].env_name, "");
    }

    // ─── Tests control_master / agent_forwarding ──────────────────────────────

    #[test]
    fn test_resolve_control_master_from_defaults() {
        let config = Config {
            defaults: Some(Defaults {
                control_master: Some(true),
                agent_forwarding: Some(true),
                control_path: Some("~/.ssh/ctl/%h_%p_%r".to_string()),
                control_persist: Some("30m".to_string()),
                ..Default::default()
            }),
            groups: vec![ConfigEntry::Group(make_group_with_env(
                "G",
                vec![make_env("E", vec![make_server("srv", "10.0.0.1", None)])],
            ))],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        let s = &resolved[0];
        assert!(s.control_master, "control_master should be true");
        assert!(s.agent_forwarding, "agent_forwarding should be true");
        assert!(
            !s.control_path.is_empty(),
            "control_path should be set when control_master=true"
        );
        assert_eq!(s.control_persist, "30m");
    }

    #[test]
    fn test_resolve_control_master_false_by_default() {
        let config = Config {
            defaults: None,
            groups: vec![ConfigEntry::Group(make_group_with_env(
                "G",
                vec![make_env("E", vec![make_server("srv", "10.0.0.1", None)])],
            ))],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        assert!(!resolved[0].control_master);
        assert!(!resolved[0].agent_forwarding);
        assert!(resolved[0].control_path.is_empty());
    }

    // ─── Tests hooks ──────────────────────────────────────────────────────────

    #[test]
    fn test_resolve_hooks_from_defaults() {
        let config = Config {
            defaults: Some(Defaults {
                pre_connect_hook: Some("/hooks/pre.sh".to_string()),
                post_disconnect_hook: Some("/hooks/post.sh".to_string()),
                hook_timeout_secs: Some(15),
                ..Default::default()
            }),
            groups: vec![ConfigEntry::Group(make_group_with_env(
                "G",
                vec![make_env("E", vec![make_server("srv", "10.0.0.1", None)])],
            ))],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        let s = &resolved[0];
        assert_eq!(s.pre_connect_hook.as_deref(), Some("/hooks/pre.sh"));
        assert_eq!(s.post_disconnect_hook.as_deref(), Some("/hooks/post.sh"));
        assert_eq!(s.hook_timeout_secs, 15);
    }

    #[test]
    fn test_resolve_server_hook_overrides_defaults() {
        let config = Config {
            defaults: Some(Defaults {
                pre_connect_hook: Some("/hooks/global.sh".to_string()),
                ..Default::default()
            }),
            groups: vec![ConfigEntry::Group(make_group_with_env(
                "G",
                vec![make_env(
                    "E",
                    vec![Server {
                        name: "srv".to_string(),
                        host: "10.0.0.1".to_string(),
                        pre_connect_hook: Some("/hooks/server.sh".to_string()),
                        ..Default::default()
                    }],
                )],
            ))],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        assert_eq!(
            resolved[0].pre_connect_hook.as_deref(),
            Some("/hooks/server.sh"),
            "server-level hook should override defaults"
        );
    }

    #[test]
    fn test_resolve_hooks_absent_by_default() {
        let config = Config {
            defaults: None,
            groups: vec![ConfigEntry::Group(make_group_with_env(
                "G",
                vec![make_env("E", vec![make_server("srv", "10.0.0.1", None)])],
            ))],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        assert!(resolved[0].pre_connect_hook.is_none());
        assert!(resolved[0].post_disconnect_hook.is_none());
        assert_eq!(resolved[0].hook_timeout_secs, 5);
    }

    // ─── Tests ssh_cert / ssh_agent_sock ─────────────────────────────────────

    #[test]
    fn test_resolve_ssh_cert_from_defaults() {
        let config = Config {
            defaults: Some(Defaults {
                ssh_cert: Some("/certs/id_ed25519-cert.pub".to_string()),
                ..Default::default()
            }),
            groups: vec![ConfigEntry::Group(make_group_with_env(
                "G",
                vec![make_env("E", vec![make_server("srv", "10.0.0.1", None)])],
            ))],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        assert_eq!(resolved[0].ssh_cert, "/certs/id_ed25519-cert.pub");
    }

    #[test]
    fn test_resolve_ssh_agent_sock_from_defaults() {
        let config = Config {
            defaults: Some(Defaults {
                ssh_agent_sock: Some("/run/user/1000/gnupg/S.gpg-agent.ssh".to_string()),
                ..Default::default()
            }),
            groups: vec![ConfigEntry::Group(make_group_with_env(
                "G",
                vec![make_env("E", vec![make_server("srv", "10.0.0.1", None)])],
            ))],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        assert_eq!(
            resolved[0].ssh_agent_sock,
            "/run/user/1000/gnupg/S.gpg-agent.ssh"
        );
    }

    #[test]
    fn test_resolve_ssh_agent_sock_server_overrides_defaults() {
        let config = Config {
            defaults: Some(Defaults {
                ssh_agent_sock: Some("/run/global.sock".to_string()),
                ..Default::default()
            }),
            groups: vec![ConfigEntry::Group(make_group_with_env(
                "G",
                vec![make_env(
                    "E",
                    vec![Server {
                        name: "srv".to_string(),
                        host: "10.0.0.1".to_string(),
                        ssh_agent_sock: Some("/run/per-server.sock".to_string()),
                        ..Default::default()
                    }],
                )],
            ))],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        assert_eq!(resolved[0].ssh_agent_sock, "/run/per-server.sock");
    }

    // ─── Tests use_system_ssh_config ─────────────────────────────────────────

    #[test]
    fn test_resolve_use_system_ssh_config_false_by_default() {
        let config = Config {
            defaults: None,
            groups: vec![ConfigEntry::Group(make_group_with_env(
                "G",
                vec![make_env("E", vec![make_server("srv", "10.0.0.1", None)])],
            ))],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        assert!(!resolved[0].use_system_ssh_config);
    }

    #[test]
    fn test_resolve_use_system_ssh_config_true_from_defaults() {
        let config = Config {
            defaults: Some(Defaults {
                use_system_ssh_config: Some(true),
                ..Default::default()
            }),
            groups: vec![ConfigEntry::Group(make_group_with_env(
                "G",
                vec![make_env("E", vec![make_server("srv", "10.0.0.1", None)])],
            ))],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        assert!(resolved[0].use_system_ssh_config);
    }

    // ─── Tests wallix field defaults ─────────────────────────────────────────

    #[test]
    fn test_resolve_wallix_field_defaults_applied() {
        let config = Config {
            defaults: Some(Defaults {
                mode: Some(ConnectionMode::Wallix),
                wallix: Some(BastionConfig {
                    host: Some("bastion.example.com".to_string()),
                    user: Some("buser".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            groups: vec![ConfigEntry::Group(make_group_with_env(
                "G",
                vec![make_env(
                    "E",
                    vec![make_server("srv", "target.example.com", None)],
                )],
            ))],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        let s = &resolved[0];
        assert!(s.wallix_auto_select, "auto_select should default to true");
        assert!(
            s.wallix_fail_if_menu_match_error,
            "fail_if_menu_match_error should default to true"
        );
        assert_eq!(s.wallix_selection_timeout_secs, 8);
        assert!(!s.wallix_direct, "direct should default to false");
        assert!(s.wallix_authorization.is_none());
        assert_eq!(s.wallix_account, "default");
        assert_eq!(s.wallix_protocol, "SSH");
    }

    #[test]
    fn test_resolve_wallix_direct_and_authorization() {
        let config = Config {
            defaults: Some(Defaults {
                mode: Some(ConnectionMode::Wallix),
                wallix: Some(BastionConfig {
                    host: Some("bastion.example.com".to_string()),
                    user: Some("buser".to_string()),
                    direct: Some(true),
                    authorization: Some("STI-TEAM_prod-admins".to_string()),
                    auto_select: Some(false),
                    selection_timeout_secs: Some(3),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            groups: vec![ConfigEntry::Group(make_group_with_env(
                "G",
                vec![make_env(
                    "E",
                    vec![make_server("srv", "target.example.com", None)],
                )],
            ))],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        let s = &resolved[0];
        assert!(s.wallix_direct);
        assert_eq!(
            s.wallix_authorization.as_deref(),
            Some("STI-TEAM_prod-admins")
        );
        assert!(!s.wallix_auto_select);
        assert_eq!(s.wallix_selection_timeout_secs, 3);
    }

    #[test]
    fn test_resolve_wallix_header_columns_custom() {
        let config = Config {
            defaults: Some(Defaults {
                mode: Some(ConnectionMode::Wallix),
                wallix: Some(BastionConfig {
                    host: Some("bastion.example.com".to_string()),
                    user: Some("buser".to_string()),
                    header_columns: Some(vec![
                        "ID".to_string(),
                        "Target".to_string(),
                        "Auth".to_string(),
                    ]),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            groups: vec![ConfigEntry::Group(make_group_with_env(
                "G",
                vec![make_env(
                    "E",
                    vec![make_server("srv", "target.example.com", None)],
                )],
            ))],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        assert_eq!(
            resolved[0].wallix_header_columns,
            vec!["ID", "Target", "Auth"]
        );
    }

    // ─── Tests jump multi-hop ─────────────────────────────────────────────────

    #[test]
    fn test_resolve_jump_multihop_string_format() {
        let config = Config {
            defaults: Some(Defaults {
                mode: Some(ConnectionMode::Jump),
                jump: Some(vec![
                    JumpConfig {
                        host: Some("hop1.example.com".to_string()),
                        user: Some("jump1".to_string()),
                    },
                    JumpConfig {
                        host: Some("hop2.example.com".to_string()),
                        user: Some("jump2".to_string()),
                    },
                ]),
                ..Default::default()
            }),
            groups: vec![ConfigEntry::Group(make_group_with_env(
                "G",
                vec![make_env("E", vec![make_server("srv", "10.0.0.1", None)])],
            ))],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        assert_eq!(
            resolved[0].jump_host.as_deref(),
            Some("jump1@hop1.example.com,jump2@hop2.example.com")
        );
    }

    #[test]
    fn test_resolve_jump_server_overrides_defaults() {
        let config = Config {
            defaults: Some(Defaults {
                mode: Some(ConnectionMode::Jump),
                jump: Some(vec![JumpConfig {
                    host: Some("default-jump.example.com".to_string()),
                    user: Some("default-jump-user".to_string()),
                }]),
                ..Default::default()
            }),
            groups: vec![ConfigEntry::Group(make_group_with_env(
                "G",
                vec![make_env(
                    "E",
                    vec![Server {
                        name: "srv".to_string(),
                        host: "10.0.0.1".to_string(),
                        jump: Some(vec![JumpConfig {
                            host: Some("custom-jump.example.com".to_string()),
                            user: Some("custom-user".to_string()),
                        }]),
                        ..Default::default()
                    }],
                )],
            ))],
            includes: vec![],
            vars: Default::default(),
        };

        let resolved = config.resolve().unwrap();
        assert_eq!(
            resolved[0].jump_host.as_deref(),
            Some("custom-user@custom-jump.example.com"),
            "server-level jump should replace defaults jump"
        );
    }

    // ─── Tests validate_yaml at group and environment level ───────────────────

    #[test]
    fn test_validation_unknown_group_field() {
        let yaml = r#"
groups:
  - name: G
    typo_group_field: true
    servers: []
"#;
        let warnings = validate_yaml(yaml, "test.yml");
        assert!(
            warnings.iter().any(|w| w.field == "typo_group_field"),
            "expected ValidationWarning for typo_group_field, got: {:?}",
            warnings
        );
    }

    #[test]
    fn test_validation_unknown_env_field() {
        let yaml = r#"
groups:
  - name: G
    environments:
      - name: E
        bad_env_key: "oops"
        servers: []
"#;
        let warnings = validate_yaml(yaml, "test.yml");
        assert!(
            warnings.iter().any(|w| w.field == "bad_env_key"),
            "expected ValidationWarning for bad_env_key, got: {:?}",
            warnings
        );
    }
}
