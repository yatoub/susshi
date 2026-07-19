use super::*;
use crate::fl;

/// Uses a non-cryptographic hash to detect config file changes.
/// Returns 0 on read error so reload logic stays conservative.
fn hash_config_file(path: &PathBuf) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    match std::fs::read(path) {
        Ok(bytes) => {
            let mut hasher = DefaultHasher::new();
            bytes.hash(&mut hasher);
            hasher.finish()
        }
        Err(_) => 0,
    }
}

impl App {
    pub fn new(
        config: Config,
        warnings: Vec<IncludeWarning>,
        config_path: PathBuf,
        validation_warnings: Vec<ValidationWarning>,
    ) -> Result<Self, ConfigError> {
        let resolved = config.resolve()?;
        let config_hash = hash_config_file(&config_path);

        let theme_variant = config
            .defaults
            .as_ref()
            .and_then(|d| d.theme)
            .unwrap_or(ThemeVariant::Mocha);

        let keep_open = config
            .defaults
            .as_ref()
            .and_then(|d| d.keep_open)
            .unwrap_or(false);

        let default_filter = config
            .defaults
            .as_ref()
            .and_then(|d| d.default_filter.clone())
            .unwrap_or_default();

        let mut app = Self {
            config,
            resolved_servers: resolved,
            selected_index: 0,
            list_state: ListState::default(),
            expanded_items: HashSet::new(),
            search_query: String::new(),
            is_searching: false,
            connection_mode: ConnectionMode::Direct,
            verbose_mode: false,
            app_mode: AppMode::Normal,
            theme: get_theme(theme_variant),
            theme_variant,
            status_message: None,
            cached_items: Vec::new(),
            items_dirty: true,
            clipboard: arboard::Clipboard::new().ok(),
            probe_state: ProbeState::Idle,
            probe_rx: None,
            warnings,
            config_path,
            config_hash,
            favorites_only: false,
            sort_by_recent: false,
            last_seen: HashMap::new(),
            favorites: HashSet::new(),
            cmd_state: CmdState::Idle,
            cmd_rx: None,
            validation_warnings,
            keep_open,
            tunnel_overrides: Vec::new(),
            tunnel_overlay: None,
            active_tunnels: HashMap::new(),
            scp_state: ScpState::Idle,
            scp_rx: None,
            wallix_selector: None,
            wallix_selector_rx: None,
            wallix_selection_cache: HashMap::new(),
            wallix_pending_connection: None,
            wallix_pending_auth: None,
            show_help: false,
            pinned_server: None,
            pinned_probe_state: ProbeState::Idle,
            pinned_probe_rx: None,
            cmd_history: Vec::new(),
            cmd_history_cursor: None,
            overview: None,
            overview_rx: None,
            mouse_capture: true,
            wizard_state: WizardState::Idle,
        };

        app.list_state.select(Some(0));

        let saved = state::load_state();
        app.expanded_items = saved.expanded_items;
        app.last_seen = saved.last_seen;
        app.favorites = saved.favorites;
        app.sort_by_recent = saved.sort_by_recent;
        app.tunnel_overrides = saved.tunnel_overrides;
        app.cmd_history = saved.command_history;
        app.items_dirty = true;

        if app.search_query.is_empty() && !default_filter.is_empty() {
            app.search_query = default_filter;
            app.is_searching = true;
            app.items_dirty = true;
        }

        app.update_mode_from_selection();

        if !app.warnings.is_empty() {
            let lines: Vec<String> = app
                .warnings
                .iter()
                .map(|w| match w {
                    crate::config::IncludeWarning::LoadError { label, path, error } => {
                        fl!(
                            "include-warn-load",
                            label = label.as_str(),
                            path = path.as_str(),
                            error = error.as_str()
                        )
                    }
                    crate::config::IncludeWarning::Circular { label, path } => {
                        fl!(
                            "include-warn-circular",
                            label = label.as_str(),
                            path = path.as_str()
                        )
                    }
                })
                .collect();
            app.app_mode = AppMode::Error(lines.join("\n"));
        }

        Ok(app)
    }

    /// Reloads configuration from disk without restarting the app.
    pub fn reload(&mut self) -> Result<(), ConfigError> {
        let mut stack = std::collections::HashSet::new();
        let (new_config, new_warnings, new_val_warnings) =
            Config::load_merged(&self.config_path, &mut stack, 0)?;
        let resolved = new_config.resolve()?;

        self.config_hash = hash_config_file(&self.config_path);

        let old_expanded = self.expanded_items.clone();
        let old_idx = self.selected_index;

        self.config = new_config;
        self.keep_open = self
            .config
            .defaults
            .as_ref()
            .and_then(|d| d.keep_open)
            .unwrap_or(false);
        self.resolved_servers = resolved;
        self.warnings = new_warnings;
        self.validation_warnings = new_val_warnings;
        self.expanded_items = old_expanded;
        self.items_dirty = true;
        self.selected_index = old_idx;
        self.list_state.select(Some(old_idx));

        let count = self.resolved_servers.len();
        self.set_status_message(fl!("config-reloaded", count = (count as i64)));
        Ok(())
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.stop_all_tunnels();
        drop(self.scp_rx.take());
    }
}
