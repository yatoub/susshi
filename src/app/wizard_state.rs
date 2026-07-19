use super::*;
use crate::fl;

impl App {
    /// Démarre le wizard de première configuration.
    pub fn start_wizard(&mut self) {
        self.wizard_state = WizardState::Form {
            form: WizardForm::default(),
            focus: WizardField::GroupName,
            error: None,
        };
    }

    fn wizard_focused_field(&mut self) -> Option<&mut String> {
        if let WizardState::Form { form, focus, .. } = &mut self.wizard_state {
            Some(match focus {
                WizardField::GroupName => &mut form.group_name,
                WizardField::ServerName => &mut form.server_name,
                WizardField::Host => &mut form.host,
                WizardField::User => &mut form.user,
            })
        } else {
            None
        }
    }

    pub fn wizard_push_char(&mut self, c: char) {
        if let WizardState::Form { error, .. } = &mut self.wizard_state {
            *error = None;
        }
        if let Some(field) = self.wizard_focused_field() {
            field.push(c);
        }
    }

    pub fn wizard_backspace(&mut self) {
        if let WizardState::Form { error, .. } = &mut self.wizard_state {
            *error = None;
        }
        if let Some(field) = self.wizard_focused_field() {
            field.pop();
        }
    }

    pub fn wizard_next_field(&mut self) {
        if let WizardState::Form { focus, .. } = &mut self.wizard_state {
            *focus = focus.next();
        }
    }

    pub fn wizard_prev_field(&mut self) {
        if let WizardState::Form { focus, .. } = &mut self.wizard_state {
            *focus = focus.prev();
        }
    }

    /// Ferme le wizard sans écrire de serveur : `~/.susshi.yml` reste tel
    /// qu'écrit au premier lancement (squelette `groups: []`).
    pub fn wizard_cancel(&mut self) {
        self.wizard_state = WizardState::Idle;
    }

    /// Valide le formulaire, écrit le groupe/serveur dans `~/.susshi.yml` et
    /// recharge la config. En cas de champ requis manquant, affiche une
    /// erreur dans l'overlay sans fermer le wizard.
    pub fn wizard_submit(&mut self) -> Result<(), ConfigError> {
        let WizardState::Form { form, error, .. } = &mut self.wizard_state else {
            return Ok(());
        };

        if form.group_name.trim().is_empty() {
            *error = Some(fl!("wizard-error-group-name"));
            return Ok(());
        }
        if form.server_name.trim().is_empty() {
            *error = Some(fl!("wizard-error-server-name"));
            return Ok(());
        }
        if form.host.trim().is_empty() {
            *error = Some(fl!("wizard-error-host"));
            return Ok(());
        }

        let yaml = render_group_yaml(form);
        std::fs::write(&self.config_path, yaml)?;

        self.wizard_state = WizardState::Idle;
        self.reload()?;
        self.set_status_message(fl!("wizard-created"));
        Ok(())
    }
}

/// Construit le contenu YAML complet de `~/.susshi.yml` pour un unique
/// groupe/serveur saisi via le wizard. Fonction pure — testable sans I/O.
pub fn render_group_yaml(form: &WizardForm) -> String {
    let mut out = String::new();
    out.push_str("groups:\n");
    out.push_str(&format!(
        "  - name: \"{}\"\n",
        escape_yaml(&form.group_name)
    ));
    out.push_str("    servers:\n");
    out.push_str(&format!(
        "      - name: \"{}\"\n",
        escape_yaml(&form.server_name)
    ));
    out.push_str(&format!("        host: \"{}\"\n", escape_yaml(&form.host)));
    if !form.user.trim().is_empty() {
        out.push_str(&format!("        user: \"{}\"\n", escape_yaml(&form.user)));
    }
    out
}

fn escape_yaml(s: &str) -> String {
    s.trim().replace('\\', "\\\\").replace('"', "\\\"")
}
