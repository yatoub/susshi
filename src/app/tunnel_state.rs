use super::*;
use crate::fl;

impl App {
    /// Retourne la liste effective des tunnels pour un serveur :
    /// tunnels YAML fusionnes avec les overrides utilisateur persistants.
    pub fn effective_tunnels(&self, server: &ResolvedServer) -> Vec<state::EffectiveTunnel> {
        state::effective_tunnels_for(
            &server.tunnels,
            &Self::server_key(server),
            &self.tunnel_overrides,
        )
    }

    /// Ajoute un tunnel cree manuellement depuis la TUI pour un serveur donne.
    /// Persiste immediatement dans le state.
    pub fn add_tunnel_override(&mut self, server: &ResolvedServer, config: TunnelConfig) {
        self.tunnel_overrides.push(TunnelOverride {
            server_key: Self::server_key(server),
            yaml_index: None,
            config,
            hidden: false,
        });
        state::save_state(&self.to_app_state());
    }

    /// Met a jour la configuration d'un tunnel existant.
    ///
    /// - `yaml_index = Some(i)` : edition d'un tunnel YAML (cree ou met a jour l'override).
    /// - `yaml_index = None`    : edition d'un tunnel ajoute par l'utilisateur
    ///   (identifie par sa position `user_idx` parmi les overrides sans yaml_index).
    pub fn update_tunnel_override(
        &mut self,
        server: &ResolvedServer,
        yaml_index: Option<usize>,
        user_idx: usize,
        new_config: TunnelConfig,
    ) {
        let key = Self::server_key(server);
        match yaml_index {
            Some(i) => {
                // Cree ou met a jour l'override pour le tunnel YAML #i.
                if let Some(o) = self
                    .tunnel_overrides
                    .iter_mut()
                    .find(|o| o.server_key == key && o.yaml_index == Some(i))
                {
                    o.config = new_config;
                    o.hidden = false;
                } else {
                    self.tunnel_overrides.push(TunnelOverride {
                        server_key: key,
                        yaml_index: Some(i),
                        config: new_config,
                        hidden: false,
                    });
                }
            }
            None => {
                // Trouve le tunnel utilisateur par sa position parmi les overrides sans yaml_index.
                let mut count = 0usize;
                for o in self.tunnel_overrides.iter_mut() {
                    if o.server_key == key && o.yaml_index.is_none() && !o.hidden {
                        if count == user_idx {
                            o.config = new_config;
                            break;
                        }
                        count += 1;
                    }
                }
            }
        }
        state::save_state(&self.to_app_state());
    }

    /// Supprime ou masque un tunnel.
    ///
    /// - Tunnel YAML (`yaml_index = Some(i)`) : marque `hidden = true` dans l'override
    ///   (le tunnel reste masque apres rechargement du YAML).
    /// - Tunnel utilisateur (`yaml_index = None`) : retire l'entree des overrides.
    pub fn remove_tunnel_override(
        &mut self,
        server: &ResolvedServer,
        yaml_index: Option<usize>,
        user_idx: usize,
    ) {
        let key = Self::server_key(server);
        match yaml_index {
            Some(i) => {
                if let Some(o) = self
                    .tunnel_overrides
                    .iter_mut()
                    .find(|o| o.server_key == key && o.yaml_index == Some(i))
                {
                    o.hidden = true;
                } else {
                    self.tunnel_overrides.push(TunnelOverride {
                        server_key: key,
                        yaml_index: Some(i),
                        config: server.tunnels[i].clone(),
                        hidden: true,
                    });
                }
            }
            None => {
                let mut count = 0usize;
                self.tunnel_overrides.retain(|o| {
                    if o.server_key == key && o.yaml_index.is_none() && !o.hidden {
                        if count == user_idx {
                            count += 1;
                            return false; // retire cet element
                        }
                        count += 1;
                    }
                    true
                });
            }
        }
        state::save_state(&self.to_app_state());
    }

    /// Demarre un tunnel SSH pour le serveur courant a l'index effectif `effective_idx`.
    ///
    /// Sans effet (avec message d'avertissement) en mode Wallix sans autorisation
    /// connue par la config, ou si le tunnel est deja en cours d'execution.
    pub fn start_tunnel(&mut self, server: &ResolvedServer, effective_idx: usize) {
        if self.connection_mode == ConnectionMode::Wallix
            && !ssh_tunnel::wallix_tunnel_available(server)
        {
            self.set_status_message(fl!("tunnel-wallix-unavailable"));
            return;
        }

        let tunnels = self.effective_tunnels(server);
        let Some(et) = tunnels.get(effective_idx) else {
            self.set_status_message(fl!("tunnel-not-found", index = (effective_idx as i64)));
            return;
        };

        let key = Self::server_key(server);

        // Verifie si deja actif.
        if let Some(handles) = self.active_tunnels.get(&key)
            && let Some(h) = handles.iter().find(|h| h.user_idx == effective_idx)
            && h.is_running()
        {
            let port_str = et.config.local_port.to_string();
            self.set_status_message(fl!(
                "tunnel-already-active",
                label = et.config.label.as_str(),
                port = port_str.as_str()
            ));
            return;
        }

        let config = et.config.clone();
        let yaml_index = et.yaml_index;
        let label = config.label.clone();
        let local_port = config.local_port;

        match ssh_tunnel::spawn_tunnel(
            server,
            self.connection_mode,
            config,
            yaml_index,
            effective_idx,
        ) {
            Ok(handle) => {
                self.active_tunnels
                    .entry(key)
                    .or_default()
                    .retain(|h| h.user_idx != effective_idx); // retire l'ancien handle si present
                self.active_tunnels
                    .entry(Self::server_key(server))
                    .or_default()
                    .push(handle);
                let port_str = local_port.to_string();
                self.set_status_message(fl!(
                    "tunnel-started",
                    label = label.as_str(),
                    port = port_str.as_str()
                ));
            }
            Err(e) => {
                let err = e.to_string();
                self.set_status_message(fl!("tunnel-error", error = err.as_str()));
            }
        }
    }

    /// Arrete un tunnel actif identifie par `effective_idx`.
    pub fn stop_tunnel(&mut self, server_key: &str, effective_idx: usize) {
        if let Some(handles) = self.active_tunnels.get_mut(server_key)
            && let Some(h) = handles.iter_mut().find(|h| h.user_idx == effective_idx)
        {
            let label = h.config.label.clone();
            let port = h.config.local_port;
            h.kill();
            let port_str = port.to_string();
            self.set_status_message(fl!(
                "tunnel-stopped",
                label = label.as_str(),
                port = port_str.as_str()
            ));
        }
    }

    /// Arrete tous les tunnels actifs (appele au Drop et lors du rechargement).
    pub fn stop_all_tunnels(&mut self) {
        for handles in self.active_tunnels.values_mut() {
            for h in handles.iter_mut() {
                h.kill();
            }
        }
    }

    /// Sonde l'etat de tous les tunnels actifs.
    ///
    /// A appeler depuis la boucle d'evenements (tick) pour detecter les tunnels
    /// qui se sont arretes inopinement. Affiche un message de statut pour chaque
    /// tunnel mort depuis le dernier appel.
    pub fn poll_tunnel_events(&mut self) {
        let mut dead: Vec<(String, String, u16)> = Vec::new(); // (reason, label, port)

        for handles in self.active_tunnels.values_mut() {
            for h in handles.iter_mut() {
                if h.poll()
                    && let TunnelStatus::Dead(reason) = &h.status
                {
                    dead.push((reason.clone(), h.config.label.clone(), h.config.local_port));
                }
            }
        }

        for (reason, label, port) in dead {
            let port_str = port.to_string();
            self.set_status_message(fl!(
                "tunnel-died",
                label = label.as_str(),
                port = port_str.as_str(),
                reason = reason.as_str()
            ));
        }
    }

    /// Retourne le nombre de tunnels SSH actuellement actifs pour un serveur.
    pub fn active_tunnel_count(&self, server: &ResolvedServer) -> usize {
        let key = Self::server_key(server);
        self.active_tunnels
            .get(&key)
            .map(|handles| handles.iter().filter(|h| h.is_running()).count())
            .unwrap_or(0)
    }

    /// Ouvre l'overlay des tunnels pour le serveur selectionne.
    ///
    /// Sans effet si aucun serveur n'est selectionne ou si le mode Wallix est actif.
    pub fn open_tunnel_overlay(&mut self) {
        if self.connection_mode == ConnectionMode::Wallix
            && !self
                .selected_server()
                .is_some_and(|s| ssh_tunnel::wallix_tunnel_available(&s))
        {
            self.set_status_message(fl!("tunnel-wallix-unavailable"));
            return;
        }
        if self.selected_server().is_some() {
            self.tunnel_overlay = Some(TunnelOverlayState::List { selected: 0 });
        }
    }

    /// Ferme l'overlay des tunnels.
    pub fn close_tunnel_overlay(&mut self) {
        self.tunnel_overlay = None;
    }

    /// Deplace la selection vers le bas dans l'overlay.
    pub fn tunnel_overlay_next(&mut self) {
        let server = match self.selected_server() {
            Some(s) => s,
            None => return,
        };
        let count = self.effective_tunnels(&server).len() + 1; // +1 pour la ligne "+"
        if let Some(TunnelOverlayState::List { selected }) = &mut self.tunnel_overlay
            && count > 0
        {
            *selected = (*selected + 1) % count;
        }
    }

    /// Deplace la selection vers le haut dans l'overlay.
    pub fn tunnel_overlay_previous(&mut self) {
        let server = match self.selected_server() {
            Some(s) => s,
            None => return,
        };
        let count = self.effective_tunnels(&server).len() + 1;
        if let Some(TunnelOverlayState::List { selected }) = &mut self.tunnel_overlay
            && count > 0
        {
            if *selected == 0 {
                *selected = count - 1;
            } else {
                *selected -= 1;
            }
        }
    }

    /// Bascule l'etat (demarrer/arreter) du tunnel selectionne dans l'overlay.
    ///
    /// Si la ligne selectionnee est le bouton « + », sans effet (Step 5).
    pub fn tunnel_overlay_toggle(&mut self) {
        let server = match self.selected_server() {
            Some(s) => s,
            None => return,
        };
        let selected = match &self.tunnel_overlay {
            Some(TunnelOverlayState::List { selected }) => *selected,
            _ => return,
        };
        let tunnels = self.effective_tunnels(&server);
        if selected >= tunnels.len() {
            return; // bouton "+" - Step 5
        }
        let key = Self::server_key(&server);
        let is_running = self
            .active_tunnels
            .get(&key)
            .and_then(|handles| handles.iter().find(|h| h.user_idx == selected))
            .map(|h| h.is_running())
            .unwrap_or(false);
        if is_running {
            self.stop_tunnel(&key, selected);
        } else {
            self.start_tunnel(&server, selected);
        }
    }

    /// Supprime (ou masque) le tunnel selectionne dans l'overlay.
    ///
    /// Arrete d'abord le tunnel s'il est actif. Ajuste la selection pour rester valide.
    pub fn tunnel_overlay_delete(&mut self) {
        let server = match self.selected_server() {
            Some(s) => s,
            None => return,
        };
        let selected = match &self.tunnel_overlay {
            Some(TunnelOverlayState::List { selected }) => *selected,
            _ => return,
        };
        let tunnels = self.effective_tunnels(&server);
        if selected >= tunnels.len() {
            return; // bouton "+"
        }
        let yaml_index = tunnels[selected].yaml_index;
        let user_idx = tunnels[selected].user_idx;
        let key = Self::server_key(&server);
        // Arret prealable si actif.
        let is_running = self
            .active_tunnels
            .get(&key)
            .and_then(|handles| handles.iter().find(|h| h.user_idx == selected))
            .map(|h| h.is_running())
            .unwrap_or(false);
        if is_running {
            self.stop_tunnel(&key, selected);
        }
        self.remove_tunnel_override(&server, yaml_index, user_idx);
        // Ajuste la selection pour rester dans les bornes.
        let new_list_len = self.effective_tunnels(&server).len();
        if let Some(TunnelOverlayState::List { selected: sel }) = &mut self.tunnel_overlay {
            if new_list_len == 0 {
                *sel = 0;
            } else if *sel >= new_list_len {
                *sel = new_list_len - 1;
            }
        }
        self.set_status_message(fl!("tunnel-deleted"));
    }

    /// Ouvre le formulaire d'edition pour le tunnel selectionne dans la liste.
    ///
    /// Sans effet si la ligne selectionnee est le bouton « + » ou si aucun serveur n'est actif.
    pub fn open_tunnel_form_edit(&mut self) {
        let server = match self.selected_server() {
            Some(s) => s,
            None => return,
        };
        let selected = match &self.tunnel_overlay {
            Some(TunnelOverlayState::List { selected }) => *selected,
            _ => return,
        };
        let tunnels = self.effective_tunnels(&server);
        if selected >= tunnels.len() {
            return; // bouton "+"
        }
        let form = TunnelForm::new_edit(selected, &tunnels[selected].config);
        self.tunnel_overlay = Some(TunnelOverlayState::Form(form));
    }

    /// Ouvre le formulaire de creation d'un nouveau tunnel.
    pub fn open_tunnel_form_add(&mut self) {
        if self.selected_server().is_none() {
            return;
        }
        if matches!(self.tunnel_overlay, Some(TunnelOverlayState::List { .. })) {
            self.tunnel_overlay = Some(TunnelOverlayState::Form(TunnelForm::new_empty()));
        }
    }

    /// Ajoute un caractere dans le champ actif du formulaire.
    pub fn tunnel_form_char(&mut self, c: char) {
        if let Some(TunnelOverlayState::Form(form)) = &mut self.tunnel_overlay {
            // Pour les champs de port, n'autoriser que les chiffres.
            if matches!(
                form.focus,
                TunnelFormField::LocalPort | TunnelFormField::RemotePort
            ) && !c.is_ascii_digit()
            {
                return;
            }
            form.current_buf_mut().push(c);
            form.error.clear();
        }
    }

    /// Supprime le dernier caractere du champ actif.
    pub fn tunnel_form_backspace(&mut self) {
        if let Some(TunnelOverlayState::Form(form)) = &mut self.tunnel_overlay {
            form.current_buf_mut().pop();
            form.error.clear();
        }
    }

    /// Passe au champ suivant (Tab).
    pub fn tunnel_form_next_field(&mut self) {
        if let Some(TunnelOverlayState::Form(form)) = &mut self.tunnel_overlay {
            form.focus = form.focus.next();
        }
    }

    /// Passe au champ precedent (Shift+Tab).
    pub fn tunnel_form_prev_field(&mut self) {
        if let Some(TunnelOverlayState::Form(form)) = &mut self.tunnel_overlay {
            form.focus = form.focus.prev();
        }
    }

    /// Valide et soumet le formulaire.
    ///
    /// En cas d'erreur de validation, stocke le message d'erreur dans `form.error` et
    /// garde le formulaire ouvert. En cas de succes, revient a la vue liste.
    pub fn tunnel_form_submit(&mut self) {
        let server = match self.selected_server() {
            Some(s) => s,
            None => return,
        };

        // Clone les donnees du formulaire pour liberer le borrow.
        let (editing_index, validation_result) =
            if let Some(TunnelOverlayState::Form(form)) = &self.tunnel_overlay {
                (form.editing_index, form.validate())
            } else {
                return;
            };

        match validation_result {
            Err(msg) => {
                // Stocke l'erreur dans le formulaire.
                if let Some(TunnelOverlayState::Form(form)) = &mut self.tunnel_overlay {
                    form.error = msg;
                }
            }
            Ok(config) => {
                match editing_index {
                    Some(idx) => {
                        // Edition - on retrouve yaml_index et user_idx depuis la liste effective.
                        let tunnels = self.effective_tunnels(&server);
                        if let Some(et) = tunnels.get(idx) {
                            let yaml_index = et.yaml_index;
                            let user_idx = et.user_idx;
                            self.update_tunnel_override(&server, yaml_index, user_idx, config);
                            self.set_status_message(fl!("tunnel-updated"));
                        }
                    }
                    None => {
                        // Creation.
                        self.add_tunnel_override(&server, config);
                        self.set_status_message(fl!("tunnel-added"));
                    }
                }
                // Revient a la liste, selection sur le dernier element ajoute/edite.
                let new_len = self.effective_tunnels(&server).len();
                let sel = match editing_index {
                    Some(idx) => idx.min(new_len.saturating_sub(1)),
                    None => new_len.saturating_sub(1),
                };
                self.tunnel_overlay = Some(TunnelOverlayState::List { selected: sel });
            }
        }
    }

    /// Annule le formulaire et revient a la vue liste (Esc).
    pub fn tunnel_form_cancel(&mut self) {
        // Recupere l'index edite pour replacer la selection au bon endroit.
        let editing_index = if let Some(TunnelOverlayState::Form(form)) = &self.tunnel_overlay {
            form.editing_index
        } else {
            return;
        };
        let server = self.selected_server();
        let sel = editing_index
            .and_then(|idx| {
                server
                    .as_ref()
                    .map(|s| idx.min(self.effective_tunnels(s).len().saturating_sub(1)))
            })
            .unwrap_or(0);
        self.tunnel_overlay = Some(TunnelOverlayState::List { selected: sel });
    }
}
