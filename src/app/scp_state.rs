use super::*;
use crate::fl;

impl App {
    /// Ouvre l'etape de selection de la direction SCP pour le serveur selectionne.
    ///
    /// Sans effet si aucun serveur n'est selectionne.
    pub fn open_scp_select_direction(&mut self) {
        if self.selected_server().is_some() {
            self.scp_state = ScpState::SelectingDirection;
        }
    }

    /// Selectionne la direction SCP et passe au formulaire.
    ///
    /// Pre-remplit le champ distant avec `user@host:~`.
    pub fn scp_select_direction(&mut self, direction: ScpDirection) {
        let Some(server) = self.selected_server() else {
            return;
        };
        let remote_default = format!("{}@{}:~", server.user, server.host);
        self.scp_state = ScpState::FillingForm {
            direction,
            local: String::new(),
            remote: remote_default,
            focus: ScpFormField::Local,
            error: String::new(),
        };
    }

    /// Annule l'overlay SCP (retour a l'etat `Idle`).
    pub fn close_scp_overlay(&mut self) {
        // N'interrompt pas un transfert en cours.
        if matches!(self.scp_state, ScpState::Running { .. }) {
            return;
        }
        self.scp_state = ScpState::Idle;
    }

    /// Confirme l'etat final SCP (Done ou Error) et retourne a `Idle`.
    pub fn dismiss_scp_result(&mut self) {
        if matches!(self.scp_state, ScpState::Done { .. } | ScpState::Error(_)) {
            self.scp_state = ScpState::Idle;
        }
    }

    /// Insere un caractere dans le champ actif du formulaire SCP.
    pub fn scp_form_char(&mut self, c: char) {
        if let ScpState::FillingForm {
            ref focus,
            ref mut local,
            ref mut remote,
            ..
        } = self.scp_state
        {
            match focus {
                ScpFormField::Local => local.push(c),
                ScpFormField::Remote => remote.push(c),
            }
        }
    }

    /// Supprime le dernier caractere du champ actif du formulaire SCP.
    pub fn scp_form_backspace(&mut self) {
        if let ScpState::FillingForm {
            ref focus,
            ref mut local,
            ref mut remote,
            ..
        } = self.scp_state
        {
            match focus {
                ScpFormField::Local => {
                    local.pop();
                }
                ScpFormField::Remote => {
                    remote.pop();
                }
            }
        }
    }

    /// Bascule le focus entre les deux champs du formulaire SCP.
    pub fn scp_form_next_field(&mut self) {
        if let ScpState::FillingForm { ref mut focus, .. } = self.scp_state {
            *focus = focus.next();
        }
    }

    /// Lance le transfert SCP (soumission du formulaire).
    ///
    /// Valide les champs, puis spawne le subprocess `scp` en arriere-plan.
    pub fn scp_form_submit(&mut self) {
        let (direction, local, remote) = match &self.scp_state {
            ScpState::FillingForm {
                direction,
                local,
                remote,
                ..
            } => (
                direction.clone(),
                local.trim().to_string(),
                remote.trim().to_string(),
            ),
            _ => return,
        };

        if local.is_empty() {
            if let ScpState::FillingForm { ref mut error, .. } = self.scp_state {
                *error = fl!("scp-form-local-required");
            }
            return;
        }
        if remote.is_empty() {
            if let ScpState::FillingForm { ref mut error, .. } = self.scp_state {
                *error = fl!("scp-form-remote-required");
            }
            return;
        }

        let Some(server) = self.selected_server() else {
            return;
        };
        let server = server.clone();
        let mode = self.connection_mode;

        // Extraction du chemin reel (sans user@host: s'il est present).
        let remote_path = if let Some((_, path)) = remote.split_once(':') {
            path.to_string()
        } else {
            remote.clone()
        };

        let label = match &direction {
            ScpDirection::Upload => local.clone(),
            ScpDirection::Download => remote_path.clone(),
        };

        match ssh_sftp::spawn_sftp(&server, mode, direction.clone(), &local, &remote_path) {
            Ok(rx) => {
                // Taille du fichier local pour l'upload ; pour le download,
                // on recevra la taille via ScpEvent::FileSize (sinon reste 0).
                let file_size = if direction == ScpDirection::Upload {
                    std::fs::metadata(&local).map(|m| m.len()).unwrap_or(0)
                } else {
                    0
                };
                self.scp_state = ScpState::Running {
                    direction,
                    label,
                    progress: 0,
                    started_at: std::time::Instant::now(),
                    file_size,
                };
                self.scp_rx = Some(rx);
            }
            Err(e) => {
                self.scp_state = ScpState::Error(e.to_string());
            }
        }
    }

    /// Sonde les evenements SCP en attente et met a jour `scp_state`.
    ///
    /// A appeler depuis la boucle d'evenements (tick).
    pub fn poll_scp_events(&mut self) {
        use std::sync::mpsc::TryRecvError;

        let Some(rx) = &self.scp_rx else { return };

        loop {
            match rx.try_recv() {
                Ok(ScpEvent::Progress(pct)) => {
                    if let ScpState::Running {
                        ref mut progress, ..
                    } = self.scp_state
                    {
                        *progress = pct;
                    }
                }
                Ok(ScpEvent::FileSize(sz)) => {
                    if let ScpState::Running {
                        ref mut file_size, ..
                    } = self.scp_state
                    {
                        *file_size = sz;
                    }
                }
                Ok(ScpEvent::Done(ok)) => {
                    if ok {
                        self.set_status_message(fl!("scp-done-ok"));
                    } else {
                        self.set_status_message(fl!("scp-done-err"));
                    }
                    let direction = if let ScpState::Running { ref direction, .. } = self.scp_state
                    {
                        direction.clone()
                    } else {
                        ScpDirection::Upload
                    };
                    self.scp_state = ScpState::Done {
                        direction,
                        exit_ok: ok,
                    };
                    self.scp_rx = None;
                    break;
                }
                Ok(ScpEvent::Error(e)) => {
                    self.set_status_message(fl!("scp-failed", error = e.as_str()));
                    self.scp_state = ScpState::Error(e);
                    self.scp_rx = None;
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    // Le thread s'est termine sans emettre Done/Error (panique) :
                    // on sort de Running pour ne pas bloquer l'etat indefiniment.
                    self.scp_rx = None;
                    if matches!(self.scp_state, ScpState::Running { .. }) {
                        let e = fl!("scp-thread-ended");
                        self.set_status_message(fl!("scp-failed", error = e.as_str()));
                        self.scp_state = ScpState::Error(e);
                    }
                    break;
                }
            }
        }
    }
}
