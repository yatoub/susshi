use super::*;

impl App {
    /// Retourne l'etat persistable de l'application (pour la sauvegarde).
    pub fn to_app_state(&self) -> crate::state::AppState {
        let mut history = self.cmd_history.clone();
        history.truncate(100);
        crate::state::AppState {
            expanded_items: self.expanded_items.clone(),
            last_seen: self.last_seen.clone(),
            favorites: self.favorites.clone(),
            sort_by_recent: self.sort_by_recent,
            tunnel_overrides: self.tunnel_overrides.clone(),
            command_history: history,
        }
    }

    /// Affiche un message temporaire dans la barre de statut.
    pub fn set_status_message(&mut self, msg: impl Into<String>) {
        self.status_message = Some((msg.into(), Instant::now()));
    }

    /// Passe en mode erreur avec le message donne.
    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.app_mode = AppMode::Error(msg.into());
    }

    /// Revient au mode normal (ferme le panneau d'erreur).
    pub fn clear_error(&mut self) {
        self.app_mode = AppMode::Normal;
    }

    /// Ouvre le dialog de saisie de credential SSH (passphrase ou mot de passe)
    /// pour le serveur et le mode de connexion actuellement sélectionnés.
    pub fn open_credential_input(&mut self, is_passphrase: bool) {
        if let Some(server) = self.selected_server() {
            self.app_mode = AppMode::CredentialInput {
                server: Box::new(server.clone()),
                mode: self.connection_mode,
                verbose: self.verbose_mode,
                is_passphrase,
                input: String::new(),
            };
        }
    }

    /// Ajoute un caractère au buffer du credential en cours de saisie.
    pub fn credential_input_push(&mut self, c: char) {
        if let AppMode::CredentialInput { input, .. } = &mut self.app_mode {
            input.push(c);
        }
    }

    /// Supprime le dernier caractère du buffer de saisie.
    pub fn credential_input_backspace(&mut self) {
        if let AppMode::CredentialInput { input, .. } = &mut self.app_mode {
            input.pop();
        }
    }

    /// Annule la saisie et revient en mode Normal.
    pub fn cancel_credential_input(&mut self) {
        self.app_mode = AppMode::Normal;
    }

    /// Valide la saisie et retourne `(server, mode, verbose, credential)` si le buffer
    /// n'est pas vide. Laisse le mode inchangé si le buffer est vide.
    pub fn submit_credential_input(
        &mut self,
    ) -> Option<(ResolvedServer, crate::config::ConnectionMode, bool, String)> {
        let AppMode::CredentialInput {
            server,
            mode,
            verbose,
            input,
            ..
        } = &self.app_mode
        else {
            return None;
        };

        if input.is_empty() {
            return None;
        }

        let result = ((**server).clone(), *mode, *verbose, input.clone());
        self.app_mode = AppMode::Normal;
        Some(result)
    }

    /// `true` si la connexion à ce serveur doit d'abord être confirmée
    /// (serveur de production ET `confirm_production` activé).
    pub fn needs_production_confirmation(&self, server: &ResolvedServer) -> bool {
        server.production && server.confirm_production
    }

    /// Ouvre la confirmation de connexion à un serveur de production.
    pub fn open_production_confirm(
        &mut self,
        server: ResolvedServer,
        mode: ConnectionMode,
        verbose: bool,
    ) {
        self.app_mode = AppMode::ConfirmProduction {
            server: Box::new(server),
            mode,
            verbose,
        };
    }

    /// Valide la confirmation : retourne `(server, mode, verbose)` et revient en mode Normal.
    /// Retourne `None` (sans toucher au mode) si aucune confirmation n'est en attente.
    pub fn accept_production_confirm(
        &mut self,
    ) -> Option<(ResolvedServer, crate::config::ConnectionMode, bool)> {
        let AppMode::ConfirmProduction {
            server,
            mode,
            verbose,
        } = &self.app_mode
        else {
            return None;
        };
        let result = ((**server).clone(), *mode, *verbose);
        self.app_mode = AppMode::Normal;
        Some(result)
    }

    /// Annule la confirmation et revient en mode Normal.
    pub fn cancel_production_confirm(&mut self) {
        self.app_mode = AppMode::Normal;
    }

    /// Calcule la cle unique d'un serveur (stable, independante de l'ordre de config).
    pub fn server_key(server: &ResolvedServer) -> String {
        let mut key = String::new();
        if !server.namespace.is_empty() {
            key.push_str("NS:");
            key.push_str(&server.namespace);
            key.push(':');
        }
        if !server.group_name.is_empty() {
            key.push_str("Group:");
            key.push_str(&server.group_name);
            key.push(':');
        }
        if !server.env_name.is_empty() {
            key.push_str("Env:");
            key.push_str(&server.env_name);
            key.push(':');
        }
        key.push_str("Server:");
        key.push_str(&server.name);
        key
    }
}
