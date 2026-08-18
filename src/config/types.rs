use super::*;

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
    #[error("File too large: '{path}' exceeds {limit} bytes")]
    FileTooLarge { path: String, limit: u64 },
    #[error("Include depth limit ({limit}) exceeded at: '{path}'")]
    IncludeDepthExceeded { path: String, limit: u32 },
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
    /// Le dépôt git contenant le fichier inclus a des commits en retard sur
    /// sa branche amont (détecté sans `git fetch`, à partir des refs locales).
    GitOutdated { path: String, behind: u32 },
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
