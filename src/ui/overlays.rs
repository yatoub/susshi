use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::app::{
    App, AppMode, ConfigItem, ScpFormField, ScpState, TunnelFormField, TunnelOverlayState,
    WallixSelectorState, WizardField, WizardState,
};
use crate::fl;
use crate::ssh::sftp::ScpDirection;
use crate::ssh::tunnel::TunnelStatus;
use crate::ui::theme::Theme;

/// Overlay affiché quand le clipboard est indisponible — montre la valeur à copier manuellement.
pub(crate) fn draw_clipboard_fallback_overlay(
    f: &mut Frame,
    value: &str,
    area: Rect,
    theme: &Theme,
) {
    let popup_w = (value.len() as u16 + 6).clamp(50, area.width.saturating_sub(4));
    let popup_area = centered_rect(popup_w, 7, area);

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Presse-papiers indisponible ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.yellow))
        .style(Style::default().bg(theme.bg));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new("Le presse-papiers est indisponible. Copiez manuellement :")
            .style(Style::default().fg(theme.subtext0)),
        chunks[0],
    );
    f.render_widget(
        Paragraph::new(value).style(
            Style::default()
                .fg(theme.green)
                .add_modifier(Modifier::BOLD),
        ),
        chunks[2],
    );
    f.render_widget(
        Paragraph::new("Entrée / Esc pour fermer").style(Style::default().fg(theme.subtext0)),
        chunks[3],
    );
}

/// Rectangle centré de taille fixe dans `area`.
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}

/// Panneau d'erreur centré, affiché par-dessus l'interface normale.
pub(crate) fn draw_error_overlay(f: &mut Frame, msg: String, area: Rect, theme: &Theme) {
    let lines: Vec<&str> = msg.lines().collect();
    let inner_h = (lines.len() as u16).max(1);
    let popup_h = inner_h + 5;
    let popup_w = (msg.lines().map(|l| l.len()).max().unwrap_or(20) as u16 + 6)
        .clamp(40, area.width.saturating_sub(4));

    let popup_area = centered_rect(popup_w, popup_h, area);

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(fl!("error-title"))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.red))
        .style(Style::default().bg(theme.bg));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    let text: Vec<Line> = lines
        .iter()
        .map(|l| Line::from(Span::styled(*l, Style::default().fg(theme.fg))))
        .collect();
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
    f.render_widget(paragraph, chunks[0]);

    let hint = Paragraph::new(fl!("error-dismiss")).style(Style::default().fg(theme.subtext0));
    f.render_widget(hint, chunks[1]);
}

pub(crate) fn draw_wallix_selector_overlay(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(state) = &app.wallix_selector else {
        return;
    };

    let popup_area = centered_rect(86, 18, area);
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(fl!("wallix-selector-title"))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.sapphire))
        .style(Style::default().bg(app.theme.bg));
    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    match state {
        WallixSelectorState::Loading { server, .. } => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(2),
                    Constraint::Min(1),
                    Constraint::Length(1),
                ])
                .split(inner);
            f.render_widget(
                Paragraph::new(fl!(
                    "wallix-selector-loading",
                    server = server.name.as_str()
                ))
                .style(Style::default().fg(app.theme.fg)),
                chunks[0],
            );
            f.render_widget(
                Paragraph::new(fl!("wallix-selector-loading-hint"))
                    .style(Style::default().fg(app.theme.subtext0))
                    .wrap(Wrap { trim: true }),
                chunks[1],
            );
            f.render_widget(
                Paragraph::new(fl!("wallix-selector-cancel-hint"))
                    .style(Style::default().fg(app.theme.subtext0)),
                chunks[2],
            );
        }
        WallixSelectorState::Error { server, message } => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(2),
                    Constraint::Min(1),
                    Constraint::Length(1),
                ])
                .split(inner);
            f.render_widget(
                Paragraph::new(fl!("wallix-selector-error", server = server.name.as_str())).style(
                    Style::default()
                        .fg(app.theme.red)
                        .add_modifier(Modifier::BOLD),
                ),
                chunks[0],
            );
            f.render_widget(
                Paragraph::new(message.clone())
                    .style(Style::default().fg(app.theme.fg))
                    .wrap(Wrap { trim: true }),
                chunks[1],
            );
            f.render_widget(
                Paragraph::new(fl!("wallix-selector-close-hint"))
                    .style(Style::default().fg(app.theme.subtext0)),
                chunks[2],
            );
        }
        WallixSelectorState::List {
            server,
            entries,
            selected,
        } => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(2),
                    Constraint::Min(6),
                    Constraint::Length(2),
                ])
                .split(inner);

            f.render_widget(
                Paragraph::new(fl!(
                    "wallix-selector-choose",
                    server = server.name.as_str(),
                    host = server.host.as_str()
                ))
                .style(Style::default().fg(app.theme.fg)),
                chunks[0],
            );

            let items: Vec<ListItem> = entries
                .iter()
                .enumerate()
                .map(|(index, entry)| {
                    let is_selected = index == *selected;
                    let bg = if is_selected {
                        app.theme.selection_bg
                    } else {
                        app.theme.bg
                    };
                    let fg = if is_selected {
                        app.theme.selection_fg
                    } else {
                        app.theme.fg
                    };
                    ListItem::new(Line::from(vec![
                        Span::styled(
                            format!("#{:<3} ", entry.id),
                            Style::default().fg(app.theme.sapphire).bg(bg),
                        ),
                        Span::styled(entry.target.clone(), Style::default().fg(fg).bg(bg)),
                        Span::styled("  →  ", Style::default().fg(app.theme.subtext0).bg(bg)),
                        Span::styled(entry.group.clone(), Style::default().fg(fg).bg(bg)),
                    ]))
                })
                .collect();
            f.render_widget(List::new(items), chunks[1]);

            f.render_widget(
                Paragraph::new(fl!("wallix-selector-list-hint"))
                    .style(Style::default().fg(app.theme.subtext0)),
                chunks[2],
            );
        }
    }
}

/// Overlay flottant centré listant les tunnels SSH d'un serveur.
pub(crate) fn draw_tunnel_overlay(f: &mut Frame, app: &mut App, area: Rect) {
    match &app.tunnel_overlay {
        Some(TunnelOverlayState::Form(_)) => {
            draw_tunnel_form(f, app, area);
            return;
        }
        Some(TunnelOverlayState::List { .. }) => {}
        None => return,
    }

    let items = app.get_visible_items();
    let server = match items.get(app.selected_index) {
        Some(ConfigItem::Server(s)) => (**s).clone(),
        _ => return,
    };
    let overlay_selected = match &app.tunnel_overlay {
        Some(TunnelOverlayState::List { selected }) => *selected,
        _ => return,
    };

    let tunnels = app.effective_tunnels(&server);
    let server_key = App::server_key(&server);
    let n_tunnels = tunnels.len();

    let content_h = (n_tunnels as u16 + 1).max(1);
    let popup_h = (content_h + 5).min(area.height.saturating_sub(4));
    let popup_w: u16 = 64.min(area.width.saturating_sub(4));
    let popup_area = centered_rect(popup_w, popup_h, area);

    f.render_widget(Clear, popup_area);

    let title = format!(" Tunnels — {} ", server.name);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.sapphire))
        .style(Style::default().bg(app.theme.bg));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(2),
        ])
        .split(inner);

    let mut list_items: Vec<ListItem> = Vec::new();

    for (i, et) in tunnels.iter().enumerate() {
        let handle = app
            .active_tunnels
            .get(&server_key)
            .and_then(|hs| hs.iter().find(|h| h.user_idx == i));

        let (icon, icon_color) = match handle {
            Some(h) if h.is_running() => ("✔", app.theme.green),
            Some(h) if matches!(h.status, TunnelStatus::Dead(_)) => ("✗", app.theme.red),
            _ => ("✖", app.theme.subtext0),
        };

        let label = if et.config.label.is_empty() {
            format!("{}:{}", et.config.remote_host, et.config.remote_port)
        } else {
            et.config.label.clone()
        };
        let route = format!(
            "localhost:{} → {}:{}",
            et.config.local_port, et.config.remote_host, et.config.remote_port
        );

        let is_sel = i == overlay_selected;
        let bg = if is_sel {
            app.theme.selection_bg
        } else {
            app.theme.bg
        };
        let fg = if is_sel {
            app.theme.selection_fg
        } else {
            app.theme.fg
        };
        let route_fg = if is_sel {
            app.theme.selection_fg
        } else {
            app.theme.subtext0
        };

        let line = Line::from(vec![
            Span::styled(format!("{} ", icon), Style::default().fg(icon_color).bg(bg)),
            Span::styled(format!("{:<20}", label), Style::default().fg(fg).bg(bg)),
            Span::styled(route, Style::default().fg(route_fg).bg(bg)),
        ]);
        list_items.push(ListItem::new(line));
    }

    let plus_sel = overlay_selected == n_tunnels;
    let plus_bg = if plus_sel {
        app.theme.selection_bg
    } else {
        app.theme.bg
    };
    let plus_fg = if plus_sel {
        app.theme.selection_fg
    } else {
        app.theme.green
    };
    list_items.push(ListItem::new(Line::from(Span::styled(
        fl!("tunnel-overlay-new"),
        Style::default().fg(plus_fg).bg(plus_bg),
    ))));

    f.render_widget(List::new(list_items), chunks[0]);

    let hint_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(chunks[2]);

    let s = Style::default().fg(app.theme.subtext0);
    f.render_widget(
        Paragraph::new(fl!("tunnel-overlay-hints1")).style(s),
        hint_chunks[0],
    );
    f.render_widget(
        Paragraph::new(fl!("tunnel-overlay-hints2")).style(s),
        hint_chunks[1],
    );
}

/// Formulaire d'édition / création d'un tunnel SSH.
fn draw_tunnel_form(f: &mut Frame, app: &mut App, area: Rect) {
    let items = app.get_visible_items();
    let server = match items.get(app.selected_index) {
        Some(ConfigItem::Server(s)) => (**s).clone(),
        _ => return,
    };
    let form = match &app.tunnel_overlay {
        Some(TunnelOverlayState::Form(form)) => form.clone(),
        _ => return,
    };

    let is_edit = form.editing_index.is_some();
    let title = if is_edit {
        fl!("tunnel-form-edit-title", server = server.name.as_str())
    } else {
        fl!("tunnel-form-new-title", server = server.name.as_str())
    };

    let popup_h: u16 = 11;
    let popup_w: u16 = 62.min(area.width.saturating_sub(4));
    let popup_area = centered_rect(popup_w, popup_h, area);

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.sapphire))
        .style(Style::default().bg(app.theme.bg));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    let label_label = fl!("tunnel-form-field-label");
    let label_lp = fl!("tunnel-form-field-local-port");
    let label_rh = fl!("tunnel-form-field-remote-host");
    let label_rp = fl!("tunnel-form-field-remote-port");

    let fields: &[(&str, &str, TunnelFormField)] = &[
        (&label_label, &form.label, TunnelFormField::Label),
        (&label_lp, &form.local_port, TunnelFormField::LocalPort),
        (&label_rh, &form.remote_host, TunnelFormField::RemoteHost),
        (&label_rp, &form.remote_port, TunnelFormField::RemotePort),
    ];

    for (i, (label, value, field)) in fields.iter().enumerate() {
        let focused = *field == form.focus;
        let (label_fg, value_bg, cursor) = if focused {
            (app.theme.sapphire, app.theme.selection_bg, "█")
        } else {
            (app.theme.subtext0, app.theme.bg, "")
        };

        let line = Line::from(vec![
            Span::styled(*label, Style::default().fg(label_fg)),
            Span::styled(
                format!("{}{}", value, cursor),
                Style::default().fg(app.theme.fg).bg(value_bg),
            ),
        ]);
        f.render_widget(Paragraph::new(line), chunks[i]);
    }

    if !form.error.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("  ✗ ", Style::default().fg(app.theme.red)),
                Span::styled(form.error.as_str(), Style::default().fg(app.theme.red)),
            ])),
            chunks[4],
        );
    }

    f.render_widget(
        Paragraph::new(fl!("tunnel-form-hint")).style(Style::default().fg(app.theme.subtext0)),
        chunks[6],
    );
}

/// Overlay de saisie de credential SSH (passphrase de clé ou mot de passe).
///
/// Affiché lorsque l'utilisateur appuie sur `p` avant une connexion, ou lorsque
/// le flow Wallix détecte un prompt SSH et que l'utilisateur n'a pas de credential.
pub(crate) fn draw_credential_input_overlay(f: &mut Frame, app: &App, area: Rect) {
    let AppMode::CredentialInput {
        server,
        is_passphrase,
        input,
        ..
    } = &app.app_mode
    else {
        return;
    };

    let popup_h: u16 = 6;
    let popup_w: u16 = 56.min(area.width.saturating_sub(4));
    let popup_area = centered_rect(popup_w, popup_h, area);

    f.render_widget(Clear, popup_area);

    let title = if *is_passphrase {
        fl!(
            "credential-input-title-passphrase",
            server = server.name.as_str()
        )
    } else {
        fl!(
            "credential-input-title-password",
            server = server.name.as_str()
        )
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.yellow))
        .style(Style::default().bg(app.theme.bg));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    let label = if *is_passphrase {
        fl!("credential-input-prompt-passphrase")
    } else {
        fl!("credential-input-prompt-password")
    };

    // Affiche des astérisques à la place des caractères saisis
    let masked: String = "*".repeat(input.len());
    let line = Line::from(vec![
        Span::styled(label, Style::default().fg(app.theme.yellow)),
        Span::styled(
            format!("{}█", masked),
            Style::default().fg(app.theme.fg).bg(app.theme.selection_bg),
        ),
    ]);
    f.render_widget(Paragraph::new(line), chunks[1]);

    f.render_widget(
        Paragraph::new(fl!("credential-input-hint")).style(Style::default().fg(app.theme.subtext0)),
        chunks[3],
    );
}

/// Dispatch entre la sélection de direction et le formulaire SCP.
pub(crate) fn draw_scp_overlay(f: &mut Frame, app: &mut App, area: Rect) {
    match &app.scp_state {
        ScpState::SelectingDirection => draw_scp_direction_select(f, app, area),
        ScpState::FillingForm { .. } => draw_scp_form(f, app, area),
        ScpState::Done { .. } | ScpState::Error(_) => draw_scp_result(f, app, area),
        _ => {}
    }
}

/// Petit overlay de sélection de direction (Upload / Download).
fn draw_scp_direction_select(f: &mut Frame, app: &mut App, area: Rect) {
    let items = app.get_visible_items();
    let server = match items.get(app.selected_index) {
        Some(ConfigItem::Server(s)) => s.name.clone(),
        _ => return,
    };

    let popup_h: u16 = 7;
    let popup_w: u16 = 38.min(area.width.saturating_sub(4));
    let popup_area = centered_rect(popup_w, popup_h, area);

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(fl!("scp-direction-title", server = server.as_str()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.sapphire))
        .style(Style::default().bg(app.theme.bg));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    let s_active = Style::default()
        .fg(app.theme.fg)
        .add_modifier(Modifier::BOLD);
    let s_label = Style::default().fg(app.theme.sky);
    let s_sub = Style::default().fg(app.theme.subtext0);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  ↑  ", s_active),
            Span::styled(format!("{}  ", fl!("scp-direction-upload-label")), s_label),
            Span::styled(fl!("scp-direction-upload"), s_sub),
        ])),
        chunks[0],
    );
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  ↓  ", s_active),
            Span::styled(
                format!("{}  ", fl!("scp-direction-download-label")),
                s_label,
            ),
            Span::styled(fl!("scp-direction-download"), s_sub),
        ])),
        chunks[1],
    );
    f.render_widget(
        Paragraph::new(fl!("scp-direction-hint")).style(s_sub),
        chunks[3],
    );
}

/// Formulaire SCP (deux champs : Local et Distant).
fn draw_scp_form(f: &mut Frame, app: &mut App, area: Rect) {
    let (direction, local, remote, focus, error) = match &app.scp_state {
        ScpState::FillingForm {
            direction,
            local,
            remote,
            focus,
            error,
        } => (
            direction.clone(),
            local.clone(),
            remote.clone(),
            focus.clone(),
            error.clone(),
        ),
        _ => return,
    };
    let items = app.get_visible_items();
    let server_name = match items.get(app.selected_index) {
        Some(ConfigItem::Server(s)) => s.name.clone(),
        _ => return,
    };

    let dir_label = if direction == ScpDirection::Upload {
        fl!("scp-direction-upload-label")
    } else {
        fl!("scp-direction-download-label")
    };
    let title = fl!(
        "scp-form-title",
        direction = dir_label.as_str(),
        server = server_name.as_str()
    );

    let popup_h: u16 = 8;
    let popup_w: u16 = 64.min(area.width.saturating_sub(4));
    let popup_area = centered_rect(popup_w, popup_h, area);

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.sapphire))
        .style(Style::default().bg(app.theme.bg));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    let label_local = fl!("scp-form-field-local");
    let label_remote = fl!("scp-form-field-remote");

    let fields: &[(&str, &str, ScpFormField)] = &[
        (&label_local, &local, ScpFormField::Local),
        (&label_remote, &remote, ScpFormField::Remote),
    ];

    let inner_w = popup_area.width.saturating_sub(2) as usize;

    for (i, (label, value, field)) in fields.iter().enumerate() {
        let focused = *field == focus;
        let (label_fg, value_bg, cursor) = if focused {
            (app.theme.sapphire, app.theme.selection_bg, "█")
        } else {
            (app.theme.subtext0, app.theme.bg, "")
        };

        let label_w = label.chars().count();
        let cursor_w = if focused { 1 } else { 0 };
        let max_value_w = inner_w.saturating_sub(label_w + cursor_w);
        let display_value: String = if value.len() > max_value_w && max_value_w > 1 {
            format!(
                "\u{2026}{}",
                &value[value.len().saturating_sub(max_value_w.saturating_sub(1))..]
            )
        } else {
            value.to_string()
        };

        let line = Line::from(vec![
            Span::styled(*label, Style::default().fg(label_fg)),
            Span::styled(
                format!("{}{}", display_value, cursor),
                Style::default().fg(app.theme.fg).bg(value_bg),
            ),
        ]);
        f.render_widget(Paragraph::new(line), chunks[i]);
    }

    if !error.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("  ✗ ", Style::default().fg(app.theme.red)),
                Span::styled(error, Style::default().fg(app.theme.red)),
            ])),
            chunks[2],
        );
    }

    f.render_widget(
        Paragraph::new(fl!("scp-form-hint")).style(Style::default().fg(app.theme.subtext0)),
        chunks[4],
    );
}

/// Overlay de résultat SCP (Done / Error) — ferme avec Esc / Enter / q.
fn draw_scp_result(f: &mut Frame, app: &mut App, area: Rect) {
    let (icon, color, msg) = match &app.scp_state {
        ScpState::Done { direction, exit_ok } => {
            let dir_label = if *direction == ScpDirection::Upload {
                fl!("scp-direction-upload-label")
            } else {
                fl!("scp-direction-download-label")
            };
            let icon = if *exit_ok { "✔" } else { "✗" };
            let color = if *exit_ok {
                app.theme.green
            } else {
                app.theme.red
            };
            let msg = if *exit_ok {
                fl!("scp-result-success", direction = dir_label.as_str())
            } else {
                fl!("scp-result-errors", direction = dir_label.as_str())
            };
            (icon, color, msg)
        }
        ScpState::Error(e) => (
            "✗",
            app.theme.red,
            fl!("scp-result-fail", error = e.as_str()),
        ),
        _ => return,
    };

    let popup_h: u16 = 5;
    let popup_w: u16 = (msg.len() as u16 + 8).clamp(36, area.width.saturating_sub(4));
    let popup_area = centered_rect(popup_w, popup_h, area);

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(fl!("scp-result-title"))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color))
        .style(Style::default().bg(app.theme.bg));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!("  {} ", icon), Style::default().fg(color)),
            Span::styled(msg, Style::default().fg(app.theme.fg)),
        ])),
        chunks[0],
    );
    f.render_widget(
        Paragraph::new(fl!("scp-result-hint")).style(Style::default().fg(app.theme.subtext0)),
        chunks[2],
    );
}

/// Overlay d'aide clavier affiché avec `h`.
pub(crate) fn draw_help_overlay(f: &mut Frame, area: Rect, theme: &Theme) {
    let entries: &[(&str, String)] = &[
        ("j / ↓", fl!("help-navigate-down")),
        ("k / ↑", fl!("help-navigate-up")),
        ("Enter", fl!("help-connect")),
        ("Tab", fl!("help-change-mode")),
        ("1 / 2 / 3", fl!("help-select-mode")),
        ("/", fl!("help-search")),
        ("Esc", fl!("help-close")),
        ("q", fl!("help-quit")),
        ("Space", fl!("help-expand-group")),
        ("f", fl!("help-favorite")),
        ("F", fl!("help-favorites-view")),
        ("H", fl!("help-recent-sort")),
        ("C", fl!("help-collapse-all")),
        ("E", fl!("help-expand-all")),
        ("Ctrl+Y", fl!("help-theme-toggle")),
        ("M", fl!("help-mouse-toggle")),
        ("r", fl!("help-reload")),
        ("v", fl!("help-verbose")),
        ("y", fl!("help-copy-ssh")),
        ("p", fl!("help-credential")),
        ("x", fl!("help-command")),
        ("T", fl!("help-tunnels")),
        ("s", fl!("help-scp")),
        ("d", fl!("help-probe")),
        ("o", fl!("help-overview")),
        ("|", fl!("help-pin")),
        ("h", fl!("help-help")),
        ("g / G", fl!("help-goto-top-bottom")),
        ("PgUp / PgDn", fl!("help-page")),
    ];

    let col_w: u16 = 70;
    let col_h: u16 = entries.len() as u16 + 4;
    let popup_area = centered_rect(col_w, col_h, area);

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(format!(" {} ", fl!("help-title")))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.sapphire))
        .style(Style::default().bg(theme.bg));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let lines: Vec<Line> = entries
        .iter()
        .map(|(key, desc)| {
            Line::from(vec![
                Span::styled(
                    format!("{key:>15}"),
                    Style::default()
                        .fg(theme.sapphire)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("  ", Style::default()),
                Span::styled(desc.clone(), Style::default().fg(theme.fg)),
            ])
        })
        .collect();

    f.render_widget(Paragraph::new(lines), inner);
}

/// Overlay dashboard overview — probe parallèle des serveurs d'un groupe.
pub(crate) fn draw_overview_overlay(f: &mut Frame, app: &App, area: Rect) {
    use crate::app::OverviewStatus;

    let Some(ov) = &app.overview else { return };

    // Largeur : aussi large que possible (terminal - 4), avec un minimum lisible de 90.
    let popup_w: u16 = area.width.saturating_sub(4).max(90).min(area.width);
    let popup_h: u16 = (ov.entries.len() as u16 + 3).min(area.height.saturating_sub(2));
    let popup_area = centered_rect(popup_w, popup_h, area);

    // Colonnes proportionnelles à la largeur intérieure (popup_w - 2 bordures).
    let inner_w = popup_w.saturating_sub(2) as usize;
    let name_w = (inner_w / 4).clamp(16, 30);
    let host_w = (inner_w / 5).clamp(14, 25);
    // Le reste va au détail (load/RAM/disk ou message d'erreur).
    let detail_w = inner_w.saturating_sub(2 + name_w + 1 + host_w + 2);

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(format!(
            " Overview : {}  (j/k défiler · Esc fermer) ",
            ov.group_name
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.sapphire))
        .style(Style::default().bg(app.theme.bg));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let visible_h = inner.height as usize;
    let scroll = ov.scroll.min(ov.entries.len().saturating_sub(1));

    let lines: Vec<Line> = ov
        .entries
        .iter()
        .skip(scroll)
        .take(visible_h)
        .map(|entry| {
            let (icon, color, detail) = match &entry.status {
                OverviewStatus::Pending => ("…", app.theme.subtext0, String::new()),
                OverviewStatus::Ok {
                    load,
                    ram_pct,
                    disk_pct,
                } => (
                    "✓",
                    app.theme.green,
                    format!("load: {load}  RAM: {ram_pct}%  Disk: {disk_pct}%"),
                ),
                OverviewStatus::Error(e) => {
                    let short: String = e
                        .lines()
                        .next()
                        .unwrap_or(e)
                        .chars()
                        .take(detail_w)
                        .collect();
                    ("✗", app.theme.red, short)
                }
            };
            Line::from(vec![
                Span::styled(format!("{icon} "), Style::default().fg(color)),
                Span::styled(
                    format!("{:<name_w$}", entry.server_name),
                    Style::default()
                        .fg(app.theme.fg)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" {:<host_w$} ", entry.host),
                    Style::default().fg(app.theme.subtext0),
                ),
                Span::styled(detail, Style::default().fg(color)),
            ])
        })
        .collect();

    f.render_widget(Paragraph::new(lines), inner);
}

/// Overlay du wizard de première configuration — formulaire 4 champs
/// (groupe, serveur, hôte, utilisateur), même style que `draw_scp_form`.
pub(crate) fn draw_wizard_overlay(f: &mut Frame, app: &mut App, area: Rect) {
    let (form, focus, error) = match &app.wizard_state {
        WizardState::Form { form, focus, error } => (form.clone(), *focus, error.clone()),
        WizardState::Idle => return,
    };

    let popup_h: u16 = 10;
    let popup_w: u16 = 64.min(area.width.saturating_sub(4));
    let popup_area = centered_rect(popup_w, popup_h, area);

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(fl!("wizard-title"))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.sapphire))
        .style(Style::default().bg(app.theme.bg));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    let label_group = fl!("wizard-field-group");
    let label_server = fl!("wizard-field-server");
    let label_host = fl!("wizard-field-host");
    let label_user = fl!("wizard-field-user");

    let fields: &[(&str, &str, WizardField)] = &[
        (&label_group, &form.group_name, WizardField::GroupName),
        (&label_server, &form.server_name, WizardField::ServerName),
        (&label_host, &form.host, WizardField::Host),
        (&label_user, &form.user, WizardField::User),
    ];

    let inner_w = popup_area.width.saturating_sub(2) as usize;

    for (i, (label, value, field)) in fields.iter().enumerate() {
        let focused = *field == focus;
        let (label_fg, value_bg, cursor) = if focused {
            (app.theme.sapphire, app.theme.selection_bg, "█")
        } else {
            (app.theme.subtext0, app.theme.bg, "")
        };

        let label_w = label.chars().count();
        let cursor_w = if focused { 1 } else { 0 };
        let max_value_w = inner_w.saturating_sub(label_w + cursor_w);
        let display_value: String = if value.len() > max_value_w && max_value_w > 1 {
            format!(
                "\u{2026}{}",
                &value[value.len().saturating_sub(max_value_w.saturating_sub(1))..]
            )
        } else {
            value.to_string()
        };

        let line = Line::from(vec![
            Span::styled(*label, Style::default().fg(label_fg)),
            Span::styled(
                format!("{}{}", display_value, cursor),
                Style::default().fg(app.theme.fg).bg(value_bg),
            ),
        ]);
        f.render_widget(Paragraph::new(line), chunks[i]);
    }

    if let Some(err) = &error {
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("  ✗ ", Style::default().fg(app.theme.red)),
                Span::styled(err.as_str(), Style::default().fg(app.theme.red)),
            ])),
            chunks[5],
        );
    }

    f.render_widget(
        Paragraph::new(fl!("wizard-hint")).style(Style::default().fg(app.theme.subtext0)),
        chunks[6],
    );
}
