use super::*;

impl App {
    /// Affiche ou masque l'overlay d'aide, en repartant du haut à l'ouverture.
    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
        self.help_scroll = 0;
    }

    /// Ferme l'overlay d'aide.
    pub fn close_help(&mut self) {
        self.show_help = false;
        self.help_scroll = 0;
    }

    /// Fait défiler l'aide vers le bas (borné à l'affichage par la couche UI).
    pub fn help_scroll_down(&mut self) {
        self.help_scroll = self.help_scroll.saturating_add(1);
    }

    /// Fait défiler l'aide vers le haut.
    pub fn help_scroll_up(&mut self) {
        self.help_scroll = self.help_scroll.saturating_sub(1);
    }

    pub fn next(&mut self) {
        let count = self.get_visible_items().len();
        if count > 0 {
            self.selected_index = (self.selected_index + 1) % count;
            self.list_state.select(Some(self.selected_index));
            self.update_mode_from_selection();
        }
    }

    pub fn previous(&mut self) {
        let count = self.get_visible_items().len();
        if count > 0 {
            if self.selected_index == 0 {
                self.selected_index = count - 1;
            } else {
                self.selected_index -= 1;
            }
            self.list_state.select(Some(self.selected_index));
            self.update_mode_from_selection();
        }
    }

    pub fn select(&mut self, index: usize) {
        let count = self.get_visible_items().len();
        if count > 0 && index < count {
            let changed = self.selected_index != index;
            self.selected_index = index;
            self.list_state.select(Some(self.selected_index));
            if changed {
                self.update_mode_from_selection();
            }
        }
    }

    pub(super) fn update_mode_from_selection(&mut self) {
        let items = self.get_visible_items();
        if let Some(ConfigItem::Server(server)) = items.get(self.selected_index) {
            self.connection_mode = server.default_mode;
        }
        // Reinitialise le diagnostic quand on change de serveur
        self.probe_state = ProbeState::Idle;
        self.probe_rx = None;
    }

    /// Retourne le serveur actuellement selectionne (s'il y en a un).
    pub fn selected_server(&mut self) -> Option<ResolvedServer> {
        let items = self.get_visible_items();
        if let Some(ConfigItem::Server(s)) = items.get(self.selected_index) {
            Some(*s.clone())
        } else {
            None
        }
    }
}
