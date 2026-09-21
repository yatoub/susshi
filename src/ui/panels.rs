use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Tabs, Wrap},
};

use crate::app::{App, CmdState, ConfigItem, ScpState};
use crate::fl;
use crate::probe::{ProbeProfile, ProbeState};
use crate::ssh::sftp::ScpDirection;
use crate::ui::theme::Theme;

pub(crate) fn draw_connection_mode_area(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);

    draw_tabs(f, app, chunks[0]);
    draw_verbose_toggle(f, app, chunks[1]);
}

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles = vec![fl!("tab-direct"), fl!("tab-jump"), fl!("tab-wallix")];
    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(fl!("tab-title"))
                .border_style(Style::default().fg(app.theme.border)),
        )
        .select(app.connection_mode.index())
        .style(Style::default().fg(app.theme.subtext0))
        .highlight_style(
            Style::default()
                .bg(app.theme.sky)
                .fg(app.theme.bg)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(tabs, area);
}

fn draw_verbose_toggle(f: &mut Frame, app: &App, area: Rect) {
    let checkbox = if app.verbose_mode { "☑" } else { "☐" };
    let text = format!("{} {}", checkbox, fl!("verbose-label"));

    let style = if app.verbose_mode {
        Style::default()
            .fg(app.theme.green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(app.theme.subtext0)
    };

    let verbose = Paragraph::new(text).style(style).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(fl!("verbose-title"))
            .border_style(Style::default().fg(app.theme.border)),
    );
    f.render_widget(verbose, area);
}

pub(crate) fn draw_search_bar(f: &mut Frame, app: &mut App, area: Rect) {
    let visible_items = app.get_visible_items();
    let server_count = visible_items
        .iter()
        .filter(|item| matches!(item, ConfigItem::Server(_)))
        .count();
    let total_servers = app.resolved_servers.len();

    let (search_text, title) = if app.is_searching {
        let cursor = "│";
        let text = if app.search_query.is_empty() {
            format!("{}  {}", cursor, fl!("search-placeholder"))
        } else {
            format!("{}{}", app.search_query, cursor)
        };

        let title_text = if app.search_query.is_empty() {
            fl!("search-title-active", total = (total_servers as i64))
        } else if server_count == 0 {
            fl!("search-no-results", query = app.search_query.as_str())
        } else if server_count == total_servers {
            fl!("search-all-match", count = (server_count as i64))
        } else {
            fl!(
                "search-partial",
                found = (server_count as i64),
                total = (total_servers as i64)
            )
        };

        (text, title_text)
    } else {
        let text = if app.search_query.is_empty() {
            fl!("search-idle-hint")
        } else {
            let title_text = if server_count == total_servers {
                fl!("search-result-all", count = (server_count as i64))
            } else {
                fl!(
                    "search-result-partial",
                    found = (server_count as i64),
                    total = (total_servers as i64),
                    query = app.search_query.as_str()
                )
            };
            return draw_search_with_results(
                f,
                area,
                &app.search_query,
                &title_text,
                server_count,
                app.theme,
            );
        };
        (text, fl!("search-title-idle"))
    };

    let border_color = if app.is_searching {
        app.theme.sapphire
    } else {
        app.theme.border
    };

    let text_color = if app.is_searching {
        app.theme.fg
    } else {
        app.theme.subtext0
    };

    let search = Paragraph::new(search_text)
        .style(Style::default().fg(text_color))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(border_color))
                .title(title),
        );
    f.render_widget(search, area);
}

fn draw_search_with_results(
    f: &mut Frame,
    area: Rect,
    query: &str,
    title: &str,
    count: usize,
    theme: &Theme,
) {
    let border_color = if count > 0 { theme.green } else { theme.red };

    let search = Paragraph::new(query)
        .style(Style::default().fg(theme.fg))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(border_color))
                .title(title),
        );
    f.render_widget(search, area);
}

pub(crate) fn draw_tree(f: &mut Frame, app: &mut App, area: Rect) {
    let visible_items = app.get_visible_items();
    let mut list_items = Vec::new();

    for item in &visible_items {
        let content = match item {
            ConfigItem::Namespace(label) => {
                let id = format!("NS:{}", label);
                let icon = if app.expanded_items.contains(&id) || !app.search_query.is_empty() {
                    "📦"
                } else {
                    "📫"
                };
                Line::from(vec![Span::styled(
                    format!("{} {}", icon, label),
                    Style::default()
                        .fg(app.theme.namespace_header)
                        .add_modifier(Modifier::BOLD),
                )])
            }
            ConfigItem::Group(name, ns) => {
                let id = if ns.is_empty() {
                    format!("Group:{}", name)
                } else {
                    format!("NS:{}:Group:{}", ns, name)
                };
                let icon = if app.expanded_items.contains(&id) || !app.search_query.is_empty() {
                    "📂"
                } else {
                    "📁"
                };
                let indent = if ns.is_empty() { "" } else { "  " };
                Line::from(vec![
                    Span::raw(indent),
                    Span::styled(
                        format!("{} {}", icon, name),
                        Style::default()
                            .fg(app.theme.group_header)
                            .add_modifier(Modifier::BOLD),
                    ),
                ])
            }
            ConfigItem::Environment(g, name, ns) => {
                let id = if ns.is_empty() {
                    format!("Env:{}:{}", g, name)
                } else {
                    format!("NS:{}:Env:{}:{}", ns, g, name)
                };
                let icon = if app.expanded_items.contains(&id) || !app.search_query.is_empty() {
                    "🌩️"
                } else {
                    "☁️"
                };
                let indent = if ns.is_empty() { "  " } else { "    " };
                Line::from(vec![
                    Span::raw(indent),
                    Span::styled(
                        format!("{} {}", icon, name),
                        Style::default().fg(app.theme.env_header),
                    ),
                ])
            }
            ConfigItem::Server(server) => {
                let indent = match (
                    server.namespace.is_empty(),
                    server.group_name.is_empty(),
                    server.env_name.is_empty(),
                ) {
                    (true, true, _) => "",
                    (true, false, true) => "  ",
                    (true, false, false) => "    ",
                    (false, true, _) => "  ",
                    (false, false, true) => "    ",
                    (false, false, false) => "      ",
                };
                let server_key = crate::app::App::server_key(server);
                let is_fav = app.favorites.contains(&server_key);
                let icon = if is_fav { "⭐" } else { "🖥️" };
                let name_style = if is_fav {
                    Style::default()
                        .fg(app.theme.yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(app.theme.server_item)
                };
                let mut spans = vec![
                    Span::raw(indent),
                    Span::styled(format!("{} {}", icon, server.name), name_style),
                ];
                if server.production {
                    spans.push(Span::raw(" "));
                    spans.push(production_badge(app.theme));
                }
                Line::from(spans)
            }
        };

        list_items.push(ListItem::new(content));
    }

    let list = List::new(list_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(if app.favorites_only {
                    fl!("favorites-title")
                } else {
                    fl!("panel-servers")
                })
                .border_style(Style::default().fg(app.theme.border)),
        )
        .highlight_style(
            Style::default()
                .bg(app.theme.selection_bg)
                .fg(app.theme.selection_fg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▎ ");

    f.render_stateful_widget(list, area, &mut app.list_state);
}

/// Badge rouge « PROD » affiché à côté des serveurs de production.
fn production_badge(theme: &Theme) -> Span<'static> {
    Span::styled(
        format!(" {} ", fl!("badge-production")),
        Style::default()
            .bg(theme.red)
            .fg(theme.bg)
            .add_modifier(Modifier::BOLD),
    )
}

pub(crate) fn draw_details(f: &mut Frame, app: &mut App, area: Rect) {
    let visible_items = app.get_visible_items();
    let selected_is_production = matches!(
        visible_items.get(app.selected_index),
        Some(ConfigItem::Server(s)) if s.production
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(fl!("panel-details"))
        .border_style(Style::default().fg(if selected_is_production {
            app.theme.red
        } else {
            app.theme.border
        }));

    let text = if let Some(item) = visible_items.get(app.selected_index) {
        match item {
            ConfigItem::Server(server) => {
                let port_style = if server.port != 22 {
                    Style::default()
                        .fg(app.theme.yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(app.theme.subtext0)
                };

                let mut lines = Vec::new();
                if server.production {
                    lines.push(Line::from(Span::styled(
                        fl!("label-production"),
                        Style::default()
                            .fg(app.theme.red)
                            .add_modifier(Modifier::BOLD),
                    )));
                }
                lines.extend(vec![
                    Line::from(vec![
                        Span::styled(
                            fl!("label-name"),
                            Style::default()
                                .add_modifier(Modifier::BOLD)
                                .fg(app.theme.fg),
                        ),
                        Span::raw(&server.name),
                    ]),
                    Line::from(vec![
                        Span::styled(
                            fl!("label-host"),
                            Style::default()
                                .add_modifier(Modifier::BOLD)
                                .fg(app.theme.fg),
                        ),
                        Span::raw(&server.host),
                    ]),
                    Line::from(vec![
                        Span::styled(
                            fl!("label-port"),
                            Style::default()
                                .add_modifier(Modifier::BOLD)
                                .fg(app.theme.fg),
                        ),
                        Span::styled(server.port.to_string(), port_style),
                    ]),
                    Line::from(vec![
                        Span::styled(
                            fl!("label-user"),
                            Style::default()
                                .add_modifier(Modifier::BOLD)
                                .fg(app.theme.fg),
                        ),
                        Span::raw(&server.user),
                    ]),
                    Line::from(vec![
                        Span::styled(
                            fl!("label-mode"),
                            Style::default()
                                .add_modifier(Modifier::BOLD)
                                .fg(app.theme.fg),
                        ),
                        Span::styled(
                            server.default_mode.to_string(),
                            Style::default().fg(app.theme.sapphire),
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled(
                            fl!("label-key"),
                            Style::default()
                                .add_modifier(Modifier::BOLD)
                                .fg(app.theme.fg),
                        ),
                        Span::raw(&server.ssh_key),
                    ]),
                ]);

                if let Some(jump) = &server.jump_host {
                    lines.push(Line::from(vec![
                        Span::styled(
                            fl!("label-jump"),
                            Style::default()
                                .add_modifier(Modifier::BOLD)
                                .fg(app.theme.fg),
                        ),
                        Span::styled(jump.clone(), Style::default().fg(app.theme.sky)),
                    ]));
                }

                if let Some(bhost) = &server.bastion_host {
                    let bastion_display = match &server.bastion_user {
                        Some(u) => format!("{}@{}", u, bhost),
                        None => bhost.clone(),
                    };
                    lines.push(Line::from(vec![
                        Span::styled(
                            fl!("label-wallix"),
                            Style::default()
                                .add_modifier(Modifier::BOLD)
                                .fg(app.theme.fg),
                        ),
                        Span::raw(" "),
                        Span::styled(bastion_display, Style::default().fg(app.theme.sky)),
                    ]));
                }

                if !server.ssh_options.is_empty() {
                    lines.push(Line::from(vec![Span::styled(
                        fl!("label-options"),
                        Style::default()
                            .add_modifier(Modifier::BOLD)
                            .fg(app.theme.fg),
                    )]));
                    for option in &server.ssh_options {
                        lines.push(Line::from(vec![
                            Span::raw("  \u{2022} "),
                            Span::styled(option, Style::default().fg(app.theme.subtext0)),
                        ]));
                    }
                }

                let last_seen_str = if let Some(ts) = app.last_seen_for(server) {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    let elapsed = now.saturating_sub(ts);
                    if elapsed < 60 {
                        fl!("last-seen-just-now")
                    } else {
                        let minutes = elapsed / 60;
                        let hours = minutes / 60;
                        let days = hours / 24;
                        let ago_str = if days >= 1 {
                            format!("{} j", days)
                        } else if hours >= 1 {
                            format!("{} h", hours)
                        } else {
                            format!("{} min", minutes)
                        };
                        fl!("last-seen-ago", duration = ago_str.as_str())
                    }
                } else {
                    fl!("last-seen-never")
                };
                lines.push(Line::from(vec![
                    Span::styled(
                        fl!("last-seen-label"),
                        Style::default()
                            .add_modifier(Modifier::BOLD)
                            .fg(app.theme.fg),
                    ),
                    Span::styled(last_seen_str, Style::default().fg(app.theme.subtext0)),
                ]));

                if !server.notes.is_empty() {
                    lines.push(Line::from(vec![
                        Span::styled(
                            "Notes : ",
                            Style::default()
                                .add_modifier(Modifier::BOLD)
                                .fg(app.theme.fg),
                        ),
                        Span::styled(
                            server.notes.as_str(),
                            Style::default().fg(app.theme.subtext0),
                        ),
                    ]));
                }

                {
                    let n_cfg = app.effective_tunnels(server).len();
                    let n_run = app.active_tunnel_count(server);
                    if n_cfg > 0 {
                        let (badge_fg, badge_text) = if n_run > 0 {
                            (
                                app.theme.green,
                                fl!(
                                    "tunnel-badge-active",
                                    n_run = (n_run as i64),
                                    n_cfg = (n_cfg as i64)
                                ),
                            )
                        } else {
                            (
                                app.theme.subtext0,
                                fl!("tunnel-badge-none", n_cfg = (n_cfg as i64)),
                            )
                        };
                        lines.push(Line::from(vec![
                            Span::styled(
                                fl!("tunnel-badge-label"),
                                Style::default()
                                    .add_modifier(Modifier::BOLD)
                                    .fg(app.theme.fg),
                            ),
                            Span::styled(badge_text, Style::default().fg(badge_fg)),
                        ]));
                    }
                }

                if let ScpState::Running {
                    direction,
                    label,
                    progress,
                    started_at,
                    file_size,
                } = &app.scp_state
                {
                    let dir_label = if *direction == ScpDirection::Upload {
                        fl!("scp-direction-upload-label")
                    } else {
                        fl!("scp-direction-download-label")
                    };
                    const BAR_W: usize = 20;
                    let filled = (*progress as usize * BAR_W / 100).min(BAR_W);
                    let bar_color = if *progress < 60 {
                        app.theme.green
                    } else if *progress < 85 {
                        app.theme.yellow
                    } else {
                        app.theme.sapphire
                    };
                    lines.push(Line::from(vec![Span::styled(
                        fl!("scp-in-progress", direction = dir_label.as_str()),
                        Style::default()
                            .fg(app.theme.sapphire)
                            .add_modifier(Modifier::BOLD),
                    )]));
                    let max_w = area.width.saturating_sub(4) as usize;
                    let display_label = if label.len() > max_w && max_w > 1 {
                        format!(
                            "\u{2026}{}",
                            &label[label.len().saturating_sub(max_w.saturating_sub(1))..]
                        )
                    } else {
                        label.clone()
                    };
                    lines.push(Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled(display_label, Style::default().fg(app.theme.fg)),
                    ]));

                    let elapsed_secs = started_at.elapsed().as_secs_f64();
                    let transferred = if *file_size > 0 {
                        (*progress as u64 * file_size) / 100
                    } else {
                        0
                    };
                    let speed_str = if elapsed_secs >= 1.0 && transferred > 0 {
                        let bps = transferred as f64 / elapsed_secs;
                        if bps >= 1_000_000.0 {
                            format!("{:.1} MB/s", bps / 1_000_000.0)
                        } else if bps >= 1_000.0 {
                            format!("{:.0} KB/s", bps / 1_000.0)
                        } else {
                            format!("{:.0} B/s", bps)
                        }
                    } else {
                        "-".to_string()
                    };
                    let eta_label = fl!("scp-eta-label");
                    let eta_str = if elapsed_secs >= 1.0
                        && *progress > 0
                        && *progress < 100
                        && transferred > 0
                    {
                        let remaining = file_size.saturating_sub(transferred);
                        let bps = transferred as f64 / elapsed_secs;
                        let eta_secs = (remaining as f64 / bps) as u64;
                        if eta_secs >= 3600 {
                            format!(
                                "{} {}h{:02}m",
                                eta_label,
                                eta_secs / 3600,
                                (eta_secs % 3600) / 60
                            )
                        } else if eta_secs >= 60 {
                            format!("{} {}m{:02}s", eta_label, eta_secs / 60, eta_secs % 60)
                        } else {
                            format!("{} {}s", eta_label, eta_secs)
                        }
                    } else {
                        String::new()
                    };

                    lines.push(Line::from(vec![
                        Span::styled("  [", Style::default().fg(app.theme.subtext0)),
                        Span::styled("█".repeat(filled), Style::default().fg(bar_color)),
                        Span::styled(
                            "░".repeat(BAR_W - filled),
                            Style::default().fg(app.theme.subtext0),
                        ),
                        Span::styled(
                            format!("]{:>4}%", progress),
                            Style::default().fg(app.theme.fg),
                        ),
                        Span::styled(
                            format!("  {}", speed_str),
                            Style::default().fg(app.theme.sky),
                        ),
                        Span::styled(
                            if eta_str.is_empty() {
                                String::new()
                            } else {
                                format!("  {}", eta_str)
                            },
                            Style::default().fg(app.theme.subtext0),
                        ),
                    ]));
                }

                lines.push(Line::from(""));
                match &app.cmd_state {
                    CmdState::Running(_cmd) => {
                        let ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.subsec_millis())
                            .unwrap_or(0);
                        let frames = [
                            '\u{280b}', '\u{2819}', '\u{2839}', '\u{2838}', '\u{283c}', '\u{2834}',
                            '\u{2826}', '\u{2827}', '\u{2807}', '\u{280f}',
                        ];
                        let spinner = frames[(ms / 100) as usize % frames.len()];
                        lines.push(Line::from(vec![
                            Span::styled(
                                format!("  {} ", spinner),
                                Style::default().fg(app.theme.sapphire),
                            ),
                            Span::styled(
                                fl!("cmd-running"),
                                Style::default().fg(app.theme.subtext0),
                            ),
                        ]));
                    }
                    CmdState::Done {
                        cmd,
                        output,
                        exit_ok,
                    } => {
                        let status_icon = if *exit_ok { "✔" } else { "✗" };
                        let status_color = if *exit_ok {
                            app.theme.green
                        } else {
                            app.theme.red
                        };
                        lines.push(Line::from(vec![
                            Span::styled(
                                format!("{} ", status_icon),
                                Style::default().fg(status_color),
                            ),
                            Span::styled(
                                format!("$ {}", cmd),
                                Style::default().fg(app.theme.sapphire),
                            ),
                        ]));
                        for line in output.lines().take(20) {
                            lines.push(Line::from(vec![Span::styled(
                                format!("  {}", line),
                                Style::default().fg(app.theme.fg),
                            )]));
                        }
                    }
                    CmdState::Error(e) => {
                        lines.push(Line::from(vec![
                            Span::styled("✗ ", Style::default().fg(app.theme.red)),
                            Span::raw(e.as_str()),
                        ]));
                    }
                    _ => match &app.probe_state {
                        ProbeState::Idle => {
                            lines.push(Line::from(vec![Span::styled(
                                fl!("probe-hint"),
                                Style::default().fg(app.theme.subtext0),
                            )]));
                        }
                        ProbeState::Running => {
                            let ms = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.subsec_millis())
                                .unwrap_or(0);
                            let frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
                            let spinner = frames[(ms / 100) as usize % frames.len()];
                            lines.push(Line::from(vec![
                                Span::styled(
                                    format!("  {} ", spinner),
                                    Style::default().fg(app.theme.sapphire),
                                ),
                                Span::styled(
                                    fl!("probe-running"),
                                    Style::default().fg(app.theme.subtext0),
                                ),
                            ]));
                        }
                        ProbeState::Done(r) => {
                            let theme = app.theme;
                            lines.push(Line::from(vec![Span::styled(
                                fl!("probe-section"),
                                Style::default().fg(theme.border),
                            )]));
                            if r.profile == ProbeProfile::Wallix {
                                for note in &r.notes {
                                    let style =
                                        if note.contains("error") || note.contains("<missing>") {
                                            Style::default().fg(theme.red)
                                        } else if note.contains("skipped") {
                                            Style::default().fg(theme.yellow)
                                        } else if note.contains("ok") {
                                            Style::default().fg(theme.green)
                                        } else {
                                            Style::default().fg(theme.fg)
                                        };
                                    lines.push(Line::from(vec![Span::styled(
                                        format!("  {}", note),
                                        style,
                                    )]));
                                }
                            } else {
                                lines.push(Line::from(vec![
                                    Span::styled(
                                        fl!("probe-kernel"),
                                        Style::default().add_modifier(Modifier::BOLD).fg(theme.fg),
                                    ),
                                    Span::raw(r.kernel.clone()),
                                ]));
                                lines.push(Line::from(vec![
                                    Span::styled(
                                        fl!("probe-os"),
                                        Style::default().add_modifier(Modifier::BOLD).fg(theme.fg),
                                    ),
                                    Span::raw(r.os_name.clone()),
                                ]));
                                lines.push(Line::from(vec![
                                    Span::styled(
                                        fl!("probe-cpu"),
                                        Style::default().add_modifier(Modifier::BOLD).fg(theme.fg),
                                    ),
                                    Span::raw(r.cpu_model.clone()),
                                ]));
                                lines.push(Line::from(vec![
                                    Span::styled(
                                        fl!("probe-cpu-cores"),
                                        Style::default().add_modifier(Modifier::BOLD).fg(theme.fg),
                                    ),
                                    Span::raw(r.cpu_cores.to_string()),
                                ]));
                                lines.push(Line::from(vec![
                                    Span::styled(
                                        fl!("probe-load"),
                                        Style::default().add_modifier(Modifier::BOLD).fg(theme.fg),
                                    ),
                                    Span::raw(r.load.clone()),
                                ]));
                                lines.push(probe_bar(
                                    &fl!("probe-ram"),
                                    r.ram_pct,
                                    r.ram_total_gb,
                                    theme,
                                ));
                                lines.push(probe_bar(
                                    &fl!("probe-disk"),
                                    r.disk_pct,
                                    r.disk_total_gb,
                                    theme,
                                ));
                                if server.control_master {
                                    let (cm_icon, cm_label, cm_style) = if r.control_master_active {
                                        (
                                            "⬡ ",
                                            fl!("probe-cm-active"),
                                            Style::default().fg(theme.green),
                                        )
                                    } else {
                                        (
                                            "⬡ ",
                                            fl!("probe-cm-inactive"),
                                            Style::default().fg(theme.subtext0),
                                        )
                                    };
                                    lines.push(Line::from(vec![
                                        Span::styled(
                                            fl!("probe-cm-label"),
                                            Style::default()
                                                .add_modifier(Modifier::BOLD)
                                                .fg(theme.fg),
                                        ),
                                        Span::styled(format!("{}{}", cm_icon, cm_label), cm_style),
                                    ]));
                                }
                                for fs_entry in &r.extra_fs {
                                    match &fs_entry.usage {
                                        Some(usage) => {
                                            let label = fl!(
                                                "probe-disk-extra",
                                                mount = fs_entry.mountpoint.as_str()
                                            );
                                            lines.push(probe_bar(
                                                &label,
                                                usage.pct,
                                                usage.total_gb,
                                                theme,
                                            ));
                                        }
                                        None => {
                                            lines.push(Line::from(vec![Span::styled(
                                                fl!(
                                                    "probe-fs-absent",
                                                    mount = fs_entry.mountpoint.as_str()
                                                ),
                                                Style::default()
                                                    .fg(theme.yellow)
                                                    .add_modifier(Modifier::BOLD),
                                            )]));
                                        }
                                    }
                                }
                            }
                        }
                        ProbeState::Error(msg) => {
                            lines.push(Line::from(vec![Span::styled(
                                fl!("probe-section"),
                                Style::default().fg(app.theme.border),
                            )]));
                            lines.push(Line::from(vec![
                                Span::styled("\u{2717}  ", Style::default().fg(app.theme.red)),
                                Span::raw(msg.clone()),
                            ]));
                        }
                    },
                }

                lines
            }
            ConfigItem::Namespace(label) => {
                vec![Line::from(vec![Span::styled(
                    fl!("details-namespace", label = label.as_str()),
                    Style::default()
                        .fg(app.theme.namespace_header)
                        .add_modifier(Modifier::BOLD),
                )])]
            }
            ConfigItem::Group(name, _ns) => {
                vec![Line::from(fl!("details-group", name = name.as_str()))]
            }
            ConfigItem::Environment(g, e, _ns) => {
                vec![Line::from(fl!(
                    "details-environment",
                    group = g.as_str(),
                    env = e.as_str()
                ))]
            }
        }
    } else {
        vec![Line::from(fl!("details-placeholder"))]
    };

    let paragraph = Paragraph::new(text)
        .block(block)
        .style(Style::default().fg(app.theme.fg))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

fn probe_bar(label: &str, pct: u8, total_gb: f32, theme: &Theme) -> Line<'static> {
    const BAR_WIDTH: usize = 12;
    let filled = (pct as usize * BAR_WIDTH / 100).min(BAR_WIDTH);
    let bar_color = if pct < 60 {
        theme.green
    } else if pct < 85 {
        theme.yellow
    } else {
        theme.red
    };
    Line::from(vec![
        Span::styled(
            format!("{:<9}", label),
            Style::default().add_modifier(Modifier::BOLD).fg(theme.fg),
        ),
        Span::styled("█".repeat(filled), Style::default().fg(bar_color)),
        Span::styled(
            "░".repeat(BAR_WIDTH - filled),
            Style::default().fg(theme.subtext0),
        ),
        Span::styled(
            format!("  {:>3}%  {:.1} GB", pct, total_gb),
            Style::default().fg(theme.fg),
        ),
    ])
}

pub(crate) fn draw_status_bar(f: &mut Frame, app: &App, area: Rect, selected_is_production: bool) {
    if let CmdState::Prompting(buf) = &app.cmd_state {
        let prompt = format!("{} {}\u{2588}", fl!("cmd-prompt"), buf);
        let paragraph =
            Paragraph::new(prompt).style(Style::default().bg(app.theme.bg).fg(app.theme.yellow));
        f.render_widget(paragraph, area);
        return;
    }

    if !app.mouse_capture {
        let indicator = fl!("mouse-capture-off");
        f.render_widget(
            Paragraph::new(indicator).style(
                Style::default()
                    .bg(app.theme.yellow)
                    .fg(app.theme.bg)
                    .add_modifier(Modifier::BOLD),
            ),
            area,
        );
        return;
    }

    if let Some((msg, _)) = &app.status_message {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(area);
        f.render_widget(
            Paragraph::new(msg.as_str()).style(
                Style::default()
                    .bg(app.theme.selection_bg)
                    .fg(app.theme.green),
            ),
            chunks[0],
        );
        f.render_widget(
            Paragraph::new("").style(Style::default().bg(app.theme.selection_bg)),
            chunks[1],
        );
        return;
    }

    let theme = app.theme;
    let bg = theme.selection_bg;

    let kh = |key: &str, desc: String| -> Vec<Span<'static>> {
        vec![
            Span::styled(
                format!("[{}]", key),
                Style::default()
                    .fg(theme.sky)
                    .bg(bg)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {}  ", desc),
                Style::default().fg(theme.subtext0).bg(bg),
            ),
        ]
    };

    let (line1_spans, line2_spans): (Vec<Span>, Vec<Span>) = if app.is_searching {
        (
            [
                kh("↑↓", fl!("hint-navigate")),
                kh("Esc", fl!("hint-validate-cancel")),
                kh("Ctrl+U", fl!("hint-clear")),
            ]
            .into_iter()
            .flatten()
            .collect(),
            vec![Span::styled("", Style::default().bg(bg))],
        )
    } else if !app.search_query.is_empty() {
        (
            [
                kh("↑↓", fl!("hint-navigate")),
                kh("Enter", fl!("hint-connect")),
                kh("Esc", fl!("hint-clear-filter")),
                kh("/", fl!("hint-new-search")),
                kh("q", fl!("hint-quit")),
            ]
            .into_iter()
            .flatten()
            .collect(),
            vec![Span::styled("", Style::default().bg(bg))],
        )
    } else {
        let line1 = [
            kh("Enter", fl!("hint-connect")),
            kh("Space", fl!("hint-expand")),
            kh("↑↓ jk", fl!("hint-navigate")),
            kh("/", fl!("hint-search")),
            kh("Tab 1-3", fl!("hint-mode")),
            kh("T", fl!("hint-tunnels")),
            kh("q", fl!("hint-quit")),
        ]
        .into_iter()
        .flatten()
        .collect();
        let line2 = [
            kh("d", fl!("hint-probe")),
            kh("x", fl!("hint-command")),
            kh("s", fl!("hint-scp")),
            kh("y", fl!("hint-copy-ssh")),
            kh("f", fl!("hint-favorite")),
            kh("F", fl!("hint-favorites-view")),
            kh("r", fl!("hint-reload")),
            kh("H", fl!("hint-recent-sort")),
            kh("C", fl!("hint-collapse")),
            kh("E", fl!("hint-expand-all")),
            kh("Ctrl+Y", fl!("hint-theme-toggle")),
            kh("M", fl!("hint-mouse-toggle")),
            kh("v", fl!("hint-verbose")),
        ]
        .into_iter()
        .flatten()
        .collect();
        (line1, line2)
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    let mut line1_spans = line1_spans;
    if selected_is_production {
        line1_spans.insert(0, production_badge(theme));
        line1_spans.insert(1, Span::styled(" ", Style::default().bg(bg)));
    }

    f.render_widget(
        Paragraph::new(Line::from(line1_spans)).style(Style::default().bg(bg)),
        chunks[0],
    );
    f.render_widget(
        Paragraph::new(Line::from(line2_spans)).style(Style::default().bg(bg)),
        chunks[1],
    );
}

/// Panneau droit du split pane — affiche le serveur épinglé.
pub(crate) fn draw_pinned_server(f: &mut Frame, app: &App, area: Rect) {
    let Some(server) = &app.pinned_server else {
        return;
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(format!(" 📌 {} ", server.name))
        .border_style(Style::default().fg(app.theme.yellow));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let theme = app.theme;
    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                "Name   ",
                Style::default().add_modifier(Modifier::BOLD).fg(theme.fg),
            ),
            Span::raw(server.name.as_str()),
        ]),
        Line::from(vec![
            Span::styled(
                "Host   ",
                Style::default().add_modifier(Modifier::BOLD).fg(theme.fg),
            ),
            Span::raw(server.host.as_str()),
        ]),
        Line::from(vec![
            Span::styled(
                "User   ",
                Style::default().add_modifier(Modifier::BOLD).fg(theme.fg),
            ),
            Span::raw(server.user.as_str()),
        ]),
        Line::from(vec![
            Span::styled(
                "Port   ",
                Style::default().add_modifier(Modifier::BOLD).fg(theme.fg),
            ),
            Span::raw(server.port.to_string()),
        ]),
        Line::from(vec![
            Span::styled(
                "Group  ",
                Style::default().add_modifier(Modifier::BOLD).fg(theme.fg),
            ),
            Span::raw(format!("{} / {}", server.group_name, server.env_name)),
        ]),
        Line::from(vec![
            Span::styled(
                "Mode   ",
                Style::default().add_modifier(Modifier::BOLD).fg(theme.fg),
            ),
            Span::raw(format!("{:?}", server.default_mode)),
        ]),
        Line::from(Span::styled(
            "─────────────────────────",
            Style::default().fg(theme.border),
        )),
    ];

    match &app.pinned_probe_state {
        ProbeState::Idle => {
            lines.push(Line::from(vec![Span::styled(
                fl!("probe-hint"),
                Style::default().fg(theme.subtext0),
            )]));
        }
        ProbeState::Running => {
            let ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_millis())
                .unwrap_or(0);
            let frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
            let spinner = frames[(ms / 100) as usize % frames.len()];
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {} ", spinner),
                    Style::default().fg(theme.sapphire),
                ),
                Span::styled(fl!("probe-running"), Style::default().fg(theme.subtext0)),
            ]));
        }
        ProbeState::Done(r) => {
            lines.push(Line::from(vec![Span::styled(
                fl!("probe-section"),
                Style::default().fg(theme.border),
            )]));
            if r.profile == ProbeProfile::Wallix {
                for note in &r.notes {
                    let style = if note.contains("error") || note.contains("<missing>") {
                        Style::default().fg(theme.red)
                    } else if note.contains("skipped") {
                        Style::default().fg(theme.yellow)
                    } else if note.contains("ok") {
                        Style::default().fg(theme.green)
                    } else {
                        Style::default().fg(theme.fg)
                    };
                    lines.push(Line::from(vec![Span::styled(format!("  {}", note), style)]));
                }
            } else {
                lines.push(Line::from(vec![
                    Span::styled(
                        fl!("probe-kernel"),
                        Style::default().add_modifier(Modifier::BOLD).fg(theme.fg),
                    ),
                    Span::raw(r.kernel.clone()),
                ]));
                lines.push(Line::from(vec![
                    Span::styled(
                        fl!("probe-os"),
                        Style::default().add_modifier(Modifier::BOLD).fg(theme.fg),
                    ),
                    Span::raw(r.os_name.clone()),
                ]));
                lines.push(Line::from(vec![
                    Span::styled(
                        fl!("probe-cpu"),
                        Style::default().add_modifier(Modifier::BOLD).fg(theme.fg),
                    ),
                    Span::raw(r.cpu_model.clone()),
                ]));
                lines.push(Line::from(vec![
                    Span::styled(
                        fl!("probe-cpu-cores"),
                        Style::default().add_modifier(Modifier::BOLD).fg(theme.fg),
                    ),
                    Span::raw(r.cpu_cores.to_string()),
                ]));
                lines.push(Line::from(vec![
                    Span::styled(
                        fl!("probe-load"),
                        Style::default().add_modifier(Modifier::BOLD).fg(theme.fg),
                    ),
                    Span::raw(r.load.clone()),
                ]));
                lines.push(probe_bar(
                    &fl!("probe-ram"),
                    r.ram_pct,
                    r.ram_total_gb,
                    theme,
                ));
                lines.push(probe_bar(
                    &fl!("probe-disk"),
                    r.disk_pct,
                    r.disk_total_gb,
                    theme,
                ));
                for fs_entry in &r.extra_fs {
                    match &fs_entry.usage {
                        Some(usage) => {
                            let label =
                                fl!("probe-disk-extra", mount = fs_entry.mountpoint.as_str());
                            lines.push(probe_bar(&label, usage.pct, usage.total_gb, theme));
                        }
                        None => {
                            lines.push(Line::from(vec![Span::styled(
                                fl!("probe-fs-absent", mount = fs_entry.mountpoint.as_str()),
                                Style::default()
                                    .fg(theme.yellow)
                                    .add_modifier(Modifier::BOLD),
                            )]));
                        }
                    }
                }
            }
        }
        ProbeState::Error(msg) => {
            lines.push(Line::from(vec![Span::styled(
                fl!("probe-section"),
                Style::default().fg(theme.border),
            )]));
            lines.push(Line::from(vec![
                Span::styled("\u{2717}  ", Style::default().fg(theme.red)),
                Span::raw(msg.clone()),
            ]));
        }
    }

    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}
