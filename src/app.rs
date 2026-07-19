use crate::config::{
    Config, ConfigEntry, ConfigError, ConnectionMode, IncludeWarning, ResolvedServer, ThemeVariant,
    TunnelConfig, ValidationWarning,
};
use crate::fl;
use crate::probe::{ProbeResult, ProbeState};
use crate::ssh::sftp::{self as ssh_sftp, ScpDirection, ScpEvent};
use crate::ssh::tunnel::{self as ssh_tunnel, TunnelHandle, TunnelStatus};
use crate::state::{self, TunnelOverride};
use crate::ui::theme::{Theme, get_theme};
use crate::wallix::{WallixMenuEntry, build_expected_targets, select_id_for_server};
use ratatui::widgets::ListState;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Instant;

#[path = "app/search.rs"]
mod search;
pub use search::parse_search_tokens;

#[path = "app/visible_items.rs"]
mod visible_items;

#[path = "app/selection.rs"]
mod selection;

#[path = "app/favorites.rs"]
mod favorites;

#[path = "app/command.rs"]
mod command;

#[path = "app/wallix_state.rs"]
mod wallix_state;

#[path = "app/tunnel_state.rs"]
mod tunnel_state;

#[path = "app/scp_state.rs"]
mod scp_state;

#[path = "app/lifecycle.rs"]
mod lifecycle;

#[path = "app/expansion_state.rs"]
mod expansion_state;

#[path = "app/core_state.rs"]
mod core_state;

#[path = "app/overview.rs"]
mod overview;

#[path = "app/wizard_state.rs"]
mod wizard_state;
pub use wizard_state::render_group_yaml;

/// Mode courant de l'application.
#[derive(Debug, Default)]
pub enum AppMode {
    #[default]
    Normal,
    /// Affiche un panneau d'erreur bloquant jusqu'à la confirmation.
    Error(String),
    /// Affiche la valeur qui aurait dû être copiée quand le clipboard est indisponible.
    ClipboardFallback(String),
    /// Saisie d'un credential SSH (passphrase de clé ou mot de passe) avant connexion.
    CredentialInput {
        server: Box<ResolvedServer>,
        mode: ConnectionMode,
        verbose: bool,
        /// `true` = passphrase de clé, `false` = mot de passe
        is_passphrase: bool,
        input: String,
    },
}

impl PartialEq for AppMode {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Normal, Self::Normal) | (Self::Error(_), Self::Error(_))
        )
    }
}

/// État d'une commande SSH ad-hoc (touche `x`).
#[derive(Debug, Clone, Default)]
pub enum CmdState {
    #[default]
    Idle,
    /// L'utilisateur saisit la commande (buffer).
    Prompting(String),
    /// La commande est en cours d'exécution.
    Running(String),
    /// La commande s'est terminée avec son output et code de sortie.
    Done {
        cmd: String,
        output: String,
        exit_ok: bool,
    },
    /// Erreur de lancement.
    Error(String),
}

/// Champ actif dans le formulaire d'édition/création d'un tunnel.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TunnelFormField {
    Label,
    LocalPort,
    RemoteHost,
    RemotePort,
}

impl TunnelFormField {
    /// Passe au champ suivant (cycle).
    pub fn next(&self) -> Self {
        match self {
            Self::Label => Self::LocalPort,
            Self::LocalPort => Self::RemoteHost,
            Self::RemoteHost => Self::RemotePort,
            Self::RemotePort => Self::Label,
        }
    }
    /// Passe au champ précédent (cycle).
    pub fn prev(&self) -> Self {
        match self {
            Self::Label => Self::RemotePort,
            Self::LocalPort => Self::Label,
            Self::RemoteHost => Self::LocalPort,
            Self::RemotePort => Self::RemoteHost,
        }
    }
}

/// État du formulaire d'édition ou de création d'un tunnel.
#[derive(Debug, Clone)]
pub struct TunnelForm {
    pub label: String,
    /// Saisie libre du port local (validée en `u16` à la soumission).
    pub local_port: String,
    pub remote_host: String,
    /// Saisie libre du port distant.
    pub remote_port: String,
    /// Champ en cours d'édition.
    pub focus: TunnelFormField,
    /// `Some(idx)` = édition du tunnel à cet index effectif ; `None` = création.
    pub editing_index: Option<usize>,
    /// Message d'erreur de validation (vide = pas d'erreur).
    pub error: String,
}

impl TunnelForm {
    /// Crée un formulaire vide (création d'un nouveau tunnel).
    pub fn new_empty() -> Self {
        Self {
            label: String::new(),
            local_port: String::new(),
            remote_host: String::new(),
            remote_port: String::new(),
            focus: TunnelFormField::Label,
            editing_index: None,
            error: String::new(),
        }
    }

    /// Crée un formulaire pré-rempli pour éditer le tunnel `idx`.
    pub fn new_edit(idx: usize, config: &TunnelConfig) -> Self {
        Self {
            label: config.label.clone(),
            local_port: config.local_port.to_string(),
            remote_host: config.remote_host.clone(),
            remote_port: config.remote_port.to_string(),
            focus: TunnelFormField::Label,
            editing_index: Some(idx),
            error: String::new(),
        }
    }

    /// Retourne la valeur du champ courant (référence mutable).
    pub fn current_buf_mut(&mut self) -> &mut String {
        match self.focus {
            TunnelFormField::Label => &mut self.label,
            TunnelFormField::LocalPort => &mut self.local_port,
            TunnelFormField::RemoteHost => &mut self.remote_host,
            TunnelFormField::RemotePort => &mut self.remote_port,
        }
    }

    /// Valide les champs et retourne un `TunnelConfig` ou un message d'erreur.
    pub fn validate(&self) -> Result<TunnelConfig, String> {
        let local_port = self
            .local_port
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|&p| p >= 1)
            .ok_or_else(|| fl!("tunnel-form-local-port-invalid"))?;

        if self.remote_host.trim().is_empty() {
            return Err(fl!("tunnel-form-remote-host-empty"));
        }

        let remote_port = self
            .remote_port
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|&p| p >= 1)
            .ok_or_else(|| fl!("tunnel-form-remote-port-invalid"))?;

        Ok(TunnelConfig {
            local_port,
            remote_host: self.remote_host.trim().to_string(),
            remote_port,
            label: self.label.trim().to_string(),
        })
    }
}

/// État de l'overlay des tunnels SSH (touche `T`).
#[derive(Debug, Clone)]
pub enum TunnelOverlayState {
    /// Vue liste des tunnels configurés pour le serveur sélectionné.
    List {
        /// Index de la ligne sélectionnée (0-based ; la dernière ligne est le bouton "+").
        selected: usize,
    },
    /// Formulaire d'édition ou de création d'un tunnel.
    Form(TunnelForm),
}

/// État de l'overlay de sélection manuelle Wallix.
#[derive(Debug, Clone)]
pub enum WallixSelectorState {
    /// Récupération des entrées depuis le bastion.
    Loading {
        server: Box<ResolvedServer>,
        verbose: bool,
    },
    /// Liste des entrées Wallix disponibles.
    List {
        server: Box<ResolvedServer>,
        entries: Vec<WallixMenuEntry>,
        selected: usize,
    },
    /// Erreur lors du chargement du menu Wallix.
    Error {
        server: Box<ResolvedServer>,
        message: String,
    },
}

/// Champ actif dans le formulaire de transfert SCP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScpFormField {
    Local,
    Remote,
}

impl ScpFormField {
    pub fn next(&self) -> Self {
        match self {
            Self::Local => Self::Remote,
            Self::Remote => Self::Local,
        }
    }
    pub fn prev(&self) -> Self {
        self.next()
    }
}

/// État du transfert SCP en cours (touche `s`).
#[derive(Debug, Clone, Default)]
pub enum ScpState {
    /// Aucun transfert en cours.
    #[default]
    Idle,
    /// Sélection de la direction (Upload / Download).
    SelectingDirection,
    /// Saisie des chemins source et destination.
    FillingForm {
        direction: ScpDirection,
        /// Chemin local (peut contenir `~`).
        local: String,
        /// Chemin distant (sans le préfixe `user@host:`).
        remote: String,
        /// Champ en cours d'édition.
        focus: ScpFormField,
        /// Message d'erreur de validation.
        error: String,
    },
    /// Transfert en cours.
    Running {
        direction: ScpDirection,
        /// Nom du fichier ou chemin court affiché dans la barre.
        label: String,
        /// Progression 0–100.
        progress: u8,
        /// Instant de début du transfert (pour calculer vitesse et ETA).
        started_at: std::time::Instant,
        /// Taille totale du fichier en octets (0 si inconnue).
        file_size: u64,
    },
    /// Transfert terminé.
    Done {
        direction: ScpDirection,
        exit_ok: bool,
    },
    /// Erreur irrécupérable.
    Error(String),
}

/// Champ actif dans le formulaire du wizard de première configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardField {
    GroupName,
    ServerName,
    Host,
    User,
}

impl WizardField {
    pub fn next(self) -> Self {
        match self {
            Self::GroupName => Self::ServerName,
            Self::ServerName => Self::Host,
            Self::Host => Self::User,
            Self::User => Self::GroupName,
        }
    }
    pub fn prev(self) -> Self {
        match self {
            Self::GroupName => Self::User,
            Self::ServerName => Self::GroupName,
            Self::Host => Self::ServerName,
            Self::User => Self::Host,
        }
    }
}

/// Valeurs saisies dans le formulaire du wizard.
#[derive(Debug, Clone, Default)]
pub struct WizardForm {
    pub group_name: String,
    pub server_name: String,
    pub host: String,
    pub user: String,
}

/// État du wizard de première configuration, déclenché quand `~/.susshi.yml`
/// vient d'être créé pour la première fois (touche Esc pour l'ignorer et
/// démarrer avec une config vide).
#[derive(Debug, Default)]
pub enum WizardState {
    #[default]
    Idle,
    Form {
        form: WizardForm,
        focus: WizardField,
        error: Option<String>,
    },
}

/// Statut d'un serveur dans le dashboard overview.
#[derive(Debug, Clone)]
pub enum OverviewStatus {
    Pending,
    Ok {
        load: String,
        ram_pct: u8,
        disk_pct: u8,
    },
    Error(String),
}

/// Entrée du dashboard overview : un serveur et son statut.
#[derive(Debug, Clone)]
pub struct OverviewEntry {
    pub server_name: String,
    pub host: String,
    pub status: OverviewStatus,
}

/// État de l'overlay dashboard overview.
#[derive(Debug, Clone)]
pub struct OverviewState {
    pub group_name: String,
    pub entries: Vec<OverviewEntry>,
    pub scroll: usize,
}

#[derive(Debug, Clone)]
pub enum ConfigItem {
    /// En-tête de namespace (fichier inclus).
    Namespace(String),
    /// En-tête de groupe — `(name, ns_label)`, ns_label="" si niveau racine.
    Group(String, String),
    /// En-tête d'environnement — `(group_name, env_name, ns_label)`.
    Environment(String, String, String),
    Server(Box<ResolvedServer>),
}

pub struct App {
    pub config: Config,
    pub resolved_servers: Vec<ResolvedServer>,

    pub selected_index: usize,
    pub list_state: ListState,
    pub expanded_items: HashSet<String>,

    pub search_query: String,
    pub is_searching: bool,

    pub connection_mode: ConnectionMode,
    pub verbose_mode: bool,

    /// Mode courant (Normal ou Error).
    pub app_mode: AppMode,

    /// Thème Catppuccin actif (résolu à l'initialisation depuis la config).
    pub theme: &'static Theme,
    /// Variante de thème courante (permet le toggle à la volée).
    pub theme_variant: ThemeVariant,

    /// Message temporaire affiché dans la barre de statut (texte, timestamp)
    pub status_message: Option<(String, Instant)>,

    /// Cache de la liste visible — recalculé seulement quand `items_dirty` est vrai.
    cached_items: Vec<ConfigItem>,
    pub items_dirty: bool,

    /// Avertissements non-bloquants collectés lors du chargement des includes.
    pub warnings: Vec<IncludeWarning>,

    /// Instance du presse-papiers gardée vivante pour éviter le drop prématuré
    /// (arboard affiche un warning si l'objet est détruit trop vite après set_text).
    pub clipboard: Option<arboard::Clipboard>,

    /// État du diagnostic SSH lancé avec `d`.
    pub probe_state: ProbeState,
    /// Récepteur du thread de diagnostic (présent seulement quand Running).
    pub probe_rx: Option<mpsc::Receiver<Result<ProbeResult, String>>>,

    /// Chemin du fichier de configuration principal (pour le rechargement).
    pub config_path: PathBuf,

    /// Hash (DefaultHasher) du contenu lu sur disque — permet de détecter
    /// un rechargement inutile lorsque le fichier n'a pas été modifié.
    pub config_hash: u64,

    /// Si true, seuls les favoris sont affichés dans la liste.
    pub favorites_only: bool,

    /// Si true, la liste est triée par dernière connexion (mode plat).
    pub sort_by_recent: bool,

    /// Timestamps UNIX de dernière connexion, indexés par clé de serveur.
    pub last_seen: HashMap<String, u64>,

    /// Ensemble des clés de serveurs marqués comme favoris.
    pub favorites: HashSet<String>,

    /// État de la commande ad-hoc en cours (touche `x`).
    pub cmd_state: CmdState,

    /// Récepteur du thread de commande ad-hoc.
    pub cmd_rx: Option<mpsc::Receiver<(String, bool)>>,

    /// Avertissements de validation YAML (champs inconnus).
    pub validation_warnings: Vec<ValidationWarning>,

    /// Si true, la TUI se rouvre après la fermeture de la connexion SSH.
    pub keep_open: bool,

    /// Overrides utilisateur sur les tunnels SSH (ajouts, éditions, suppressions).
    /// Fusionnés à la volée avec `effective_tunnels()` — jamais baked dans `resolved_servers`.
    pub tunnel_overrides: Vec<TunnelOverride>,

    /// État de l'overlay tunnels. `Some(...)` = overlay ouvert, `None` = fermé.
    pub tunnel_overlay: Option<TunnelOverlayState>,

    /// Tunnels SSH actifs, indexés par clé de serveur.
    /// Chaque entrée est un `TunnelHandle` portant le sous-processus SSH et son statut.
    pub active_tunnels: HashMap<String, Vec<TunnelHandle>>,

    /// État du transfert SCP en cours.
    pub scp_state: ScpState,
    /// Récepteur des évènements du thread SFTP (présent uniquement quand Running).
    pub scp_rx: Option<mpsc::Receiver<ScpEvent>>,

    /// État de l'overlay de sélection manuelle Wallix.
    pub wallix_selector: Option<WallixSelectorState>,
    /// Récepteur du chargement asynchrone du menu Wallix.
    pub wallix_selector_rx: Option<mpsc::Receiver<WallixMenuLoadResult>>,
    /// Cache session des sélections Wallix (clé serveur -> ID choisi).
    pub wallix_selection_cache: HashMap<String, String>,
    /// Connexion Wallix prête à démarrer (résolue automatiquement ou via choix utilisateur).
    wallix_pending_connection: Option<(ResolvedServer, String)>,
    /// Credential SSH temporaire (passphrase ou mot de passe) à utiliser pour la prochaine
    /// connexion Wallix. Effacé après usage dans `take_pending_wallix_connection`.
    pub wallix_pending_auth: Option<String>,

    /// Si `true`, l'overlay d'aide clavier est affiché.
    pub show_help: bool,

    /// Serveur épinglé dans le split pane droit (None = pas de split).
    pub pinned_server: Option<Box<ResolvedServer>>,

    /// État du diagnostic SSH pour le serveur épinglé.
    pub pinned_probe_state: ProbeState,
    /// Récepteur du thread de diagnostic du serveur épinglé.
    pub pinned_probe_rx: Option<mpsc::Receiver<Result<ProbeResult, String>>>,

    /// État du dashboard overview (None = fermé).
    pub overview: Option<OverviewState>,
    /// Canal pour recevoir les résultats des probes parallèles du dashboard.
    pub overview_rx: Option<mpsc::Receiver<(usize, Result<crate::probe::ProbeResult, String>)>>,

    /// Historique des commandes ad-hoc saisies (les plus récentes en dernier).
    pub cmd_history: Vec<String>,
    /// Position du curseur dans l'historique lors de la navigation (None = pas en navigation).
    pub cmd_history_cursor: Option<usize>,

    /// Si `true`, la capture souris crossterm est active (les clics TUI fonctionnent).
    /// Si `false`, le terminal reprend la gestion souris standard (sélection texte possible).
    pub mouse_capture: bool,

    /// État du wizard de première configuration (`Idle` sauf juste après la
    /// création automatique de `~/.susshi.yml`).
    pub wizard_state: WizardState,
}

type WallixMenuLoadResult = (ResolvedServer, Result<Vec<WallixMenuEntry>, String>);

#[cfg(test)]
#[path = "app/tests_wallix.rs"]
mod tests_wallix;

#[cfg(test)]
#[path = "app/tests_wizard_state.rs"]
mod tests_wizard_state;

#[cfg(test)]
#[path = "app/tests_helpers.rs"]
mod tests_helpers;

#[cfg(test)]
#[path = "app/tests_search.rs"]
mod tests_search;

#[cfg(test)]
#[path = "app/tests_visibility.rs"]
mod tests_visibility;

#[cfg(test)]
#[path = "app/tests_credential_input.rs"]
mod tests_credential_input;

#[cfg(test)]
#[path = "app/tests_reload.rs"]
mod tests_reload;

#[cfg(test)]
#[path = "app/tests_tunnel_form.rs"]
mod tests_tunnel_form;

#[cfg(test)]
#[path = "app/tests_state_mutations.rs"]
mod tests_state_mutations;

#[cfg(test)]
#[path = "app/tests_tunnel_state.rs"]
mod tests_tunnel_state;

#[cfg(test)]
#[path = "app/tests_scp_state.rs"]
mod tests_scp_state;

#[cfg(test)]
#[path = "app/tests_overview.rs"]
mod tests_overview;

#[cfg(test)]
#[path = "app/tests_command.rs"]
mod tests_command;

#[cfg(test)]
#[path = "app/tests_selection_extended.rs"]
mod tests_selection_extended;

#[path = "app/tests_expansion_state.rs"]
#[cfg(test)]
mod tests_expansion_state;

#[path = "app/tests_favorites.rs"]
#[cfg(test)]
mod tests_favorites;
