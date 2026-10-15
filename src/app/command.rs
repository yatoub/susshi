use super::*;

impl App {
    /// Lance une commande SSH non-interactive dans un thread dedie, selon le
    /// mode de connexion courant (Direct, Jump ou Wallix).
    /// Stocke le resultat via `cmd_rx`.
    pub fn start_cmd(&mut self, server: &ResolvedServer, cmd: String) {
        if self.cmd_history.last().map(|s| s.as_str()) != Some(cmd.as_str()) {
            self.cmd_history.push(cmd.clone());
        }
        self.cmd_history_cursor = None;

        let server = server.clone();
        let mode = self.connection_mode;

        let (tx, rx) = mpsc::channel();
        self.cmd_state = CmdState::Running(cmd.clone());
        self.cmd_rx = Some(rx);

        std::thread::spawn(move || {
            let result = match crate::ssh::exec::run_remote(
                &server,
                mode,
                &cmd,
                crate::ssh::exec::COMMAND_TIMEOUT,
            ) {
                Ok(out) => (format_cmd_output(&out), out.success),
                Err(e) => (e.to_string(), false),
            };
            let _ = tx.send(result);
        });
    }

    /// Verifie si le thread de commande a produit un resultat et met a jour `cmd_state`.
    pub fn poll_cmd(&mut self) {
        let done = if let Some(rx) = &self.cmd_rx {
            rx.try_recv().ok()
        } else {
            None
        };
        if let Some((output, exit_ok)) = done {
            let cmd = match &self.cmd_state {
                CmdState::Running(c) => c.clone(),
                _ => String::new(),
            };
            self.cmd_state = CmdState::Done {
                cmd,
                output,
                exit_ok,
            };
            self.cmd_rx = None;
        }
    }

    /// Reinitialise l'etat de la commande ad-hoc.
    pub fn reset_cmd(&mut self) {
        self.cmd_state = CmdState::Idle;
        self.cmd_rx = None;
        self.cmd_history_cursor = None;
    }
}

/// Fusionne stdout et stderr pour l'affichage, separes par `---` si les deux
/// sont presents.
pub(crate) fn format_cmd_output(out: &crate::ssh::exec::ExecOutput) -> String {
    if out.stderr.is_empty() {
        out.stdout.clone()
    } else if out.stdout.is_empty() {
        out.stderr.clone()
    } else {
        format!("{}\n---\n{}", out.stdout, out.stderr)
    }
}
