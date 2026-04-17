use crate::app::{App, Overlay};
use crate::resources::CpuLevel;
use crate::tmux::IDLE_WARN_SECS;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, List, ListItem, ListState, Padding, Paragraph,
        Row, Table, TableState, Wrap,
    },
};

const ACCENT: Color = Color::Rgb(125, 87, 230);
const ATTACHED_COLOR: Color = Color::Rgb(80, 200, 120);
const DETACHED_COLOR: Color = Color::Rgb(150, 150, 150);
const IDLE_COLOR: Color = Color::Rgb(220, 160, 40);
const TITLE_FG: Color = Color::Rgb(220, 220, 255);
const SLURM_COLOR: Color = Color::Rgb(40, 180, 160);

// The flat key hints always shown in the status bar.
static HINTS: &[(&str, &str)] = &[
    ("↑/k ↓/j", "navigate"),
    ("Enter",    "attach"),
    ("n",        "new"),
    ("l",        "layout"),
    ("r",        "rename"),
    ("x",        "kill"),
    ("/",        "search"),
    ("S",        "slurm"),
    ("R",        "refresh"),
    ("q",        "quit"),
];

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Layout: header | content | status bar
    // Content splits vertically if SLURM panel is open.
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Min(0),    // content
            Constraint::Length(3), // status bar
        ])
        .split(area);

    draw_header(frame, app, root[0]);

    if app.show_slurm {
        let content = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(root[1]);
        draw_session_list(frame, app, content[0]);
        draw_slurm_panel(frame, app, content[1]);
    } else {
        draw_session_list(frame, app, root[1]);
    }

    draw_status_bar(frame, app, root[2]);

    // Overlays rendered on top of everything
    match &app.overlay {
        Overlay::NewSession => draw_input_popup(frame, "New Session", "Session name:", &app.input.clone()),
        Overlay::Rename     => draw_input_popup(frame, "Rename Session", "New name:", &app.input.clone()),
        Overlay::NewLayout  => draw_layout_picker(frame, app),
        Overlay::ConfirmKill => draw_confirm_popup(frame, app),
        Overlay::Search     => draw_search_bar(frame, app),
        Overlay::None       => {}
    }
}

// ---------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------
fn draw_header(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let session_count = app.sessions.len();
    let attached_count = app.sessions.iter().filter(|s| s.attached).count();
    let idle_count = app.sessions.iter()
        .filter(|s| !s.attached && s.idle_secs >= IDLE_WARN_SECS)
        .count();

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(22)])
        .split(area);

    // Left: branding + counts
    let mut spans = vec![
        Span::styled(" NEXUS ", Style::default().fg(TITLE_FG).bg(ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled(" by Yew Mun Yip", Style::default().fg(Color::Rgb(160, 140, 210)).add_modifier(Modifier::ITALIC)),
        Span::raw("   "),
        Span::styled(format!("{session_count} sessions"), Style::default().fg(Color::White)),
        Span::raw("  "),
        Span::styled(format!("● {attached_count} attached"), Style::default().fg(ATTACHED_COLOR)),
    ];

    if idle_count > 0 {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            format!("⏱ {idle_count} idle"),
            Style::default().fg(IDLE_COLOR).add_modifier(Modifier::BOLD),
        ));
    }

    if let Some(ref s) = app.last_session {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            format!("last: {s}"),
            Style::default().fg(Color::DarkGray),
        ));
    }

    if app.cpu.available && app.cpu.level == CpuLevel::Critical {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            " ⚠ HIGH CPU — new sessions blocked ",
            Style::default().fg(Color::Black).bg(Color::Red).add_modifier(Modifier::BOLD),
        ));
    }

    // SLURM state-change notification
    if let Some(ref note) = app.slurm_notification {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            format!(" {note} "),
            Style::default()
                .fg(Color::Black)
                .bg(SLURM_COLOR)
                .add_modifier(Modifier::BOLD),
        ));
    }

    frame.render_widget(
        Paragraph::new(Line::from(spans))
            .block(Block::default().borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(ACCENT)))
            .alignment(Alignment::Left),
        chunks[0],
    );

    // Right: CPU gauge
    let cpu_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT));

    if app.cpu.available {
        let pct = app.cpu.usage_pct.clamp(0.0, 100.0);
        let (color, label) = match app.cpu.level {
            CpuLevel::Ok       => (Color::Rgb(80, 200, 120), "CPU"),
            CpuLevel::Warning  => (Color::Yellow, "CPU ⚠"),
            CpuLevel::Critical => (Color::Red, "CPU ⚠⚠"),
        };
        frame.render_widget(
            ratatui::widgets::Gauge::default()
                .block(cpu_block.title(Span::styled(
                    format!(" {label} / {:.1}c ", app.cpu.quota_cores),
                    Style::default().fg(Color::Gray),
                )))
                .gauge_style(Style::default().fg(color).bg(Color::Rgb(30, 25, 45)))
                .percent(pct as u16)
                .label(format!("{pct:.0}%")),
            chunks[1],
        );
    } else {
        frame.render_widget(
            Paragraph::new(Span::styled(" CPU N/A ", Style::default().fg(Color::DarkGray)))
                .block(cpu_block)
                .alignment(Alignment::Center),
            chunks[1],
        );
    }
}

// ---------------------------------------------------------------------------
// Session list
// ---------------------------------------------------------------------------
fn draw_session_list(frame: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let indices = app.filtered_indices();
    let filter_active = app.overlay == Overlay::Search && !app.input.is_empty();

    let items: Vec<ListItem> = indices.iter().map(|&i| {
        let s = &app.sessions[i];
        let is_idle = !s.attached && s.idle_secs >= IDLE_WARN_SECS;

        let dot = if s.attached {
            Span::styled("● ", Style::default().fg(ATTACHED_COLOR))
        } else if is_idle {
            Span::styled("◌ ", Style::default().fg(IDLE_COLOR))
        } else {
            Span::styled("○ ", Style::default().fg(DETACHED_COLOR))
        };

        let name = Span::styled(
            s.name.clone(),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        );

        let meta = Span::styled(
            format!("  {} win", s.windows),
            Style::default().fg(Color::DarkGray),
        );

        let suffix = if s.attached {
            Span::styled("  [attached]", Style::default().fg(ATTACHED_COLOR).add_modifier(Modifier::ITALIC))
        } else if is_idle {
            Span::styled(format!("  [idle {}]", fmt_duration(s.idle_secs)), Style::default().fg(IDLE_COLOR))
        } else {
            Span::styled(format!("  {}", fmt_duration(s.idle_secs)), Style::default().fg(Color::DarkGray))
        };

        ListItem::new(Line::from(vec![Span::raw(" "), dot, name, meta, suffix]))
    }).collect();

    let title = if filter_active {
        format!(" Sessions [filter: {}] ", app.input)
    } else {
        " Sessions ".to_string()
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(ACCENT))
                .title(Span::styled(title, Style::default().fg(Color::Gray)))
                .padding(Padding::vertical(1)),
        )
        .highlight_style(
            Style::default().bg(Color::Rgb(50, 40, 80)).fg(Color::White).add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let highlight_pos = indices.iter().position(|&i| i == app.selected);
    let mut state = ListState::default();
    state.select(highlight_pos);
    frame.render_stateful_widget(list, area, &mut state);
}

// ---------------------------------------------------------------------------
// SLURM panel (shown below sessions when toggled)
// ---------------------------------------------------------------------------
fn draw_slurm_panel(frame: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    if !app.slurm_available {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled("  SLURM not available on this system (squeue not found).", Style::default().fg(Color::DarkGray))),
            ])
            .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded)
                .border_style(Style::default().fg(SLURM_COLOR))
                .title(Span::styled(" SLURM Jobs ", Style::default().fg(Color::Gray)))),
            area,
        );
        return;
    }

    if app.jobs.is_empty() {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled("  No jobs in queue.", Style::default().fg(Color::DarkGray))),
            ])
            .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded)
                .border_style(Style::default().fg(SLURM_COLOR))
                .title(Span::styled(" SLURM Jobs ", Style::default().fg(Color::Gray)))),
            area,
        );
        return;
    }

    let header = Row::new(["Job ID", "Partition", "Name", "St", "Time Used", "Time Left", "CPUs", "Mem", "Nodes", "Node/Reason"].map(|h|
        Cell::from(h).style(Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD))
    )).height(1).bottom_margin(1);

    let rows: Vec<Row> = app.jobs.iter().map(|j| {
        let state_color = j.state.color();
        Row::new(vec![
            Cell::from(j.id.clone()).style(Style::default().fg(Color::White)),
            Cell::from(j.partition.clone()).style(Style::default().fg(Color::DarkGray)),
            Cell::from(j.name.clone()).style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Cell::from(j.state.short_label().to_string()).style(Style::default().fg(state_color).add_modifier(Modifier::BOLD)),
            Cell::from(j.time_used.clone()).style(Style::default().fg(Color::White)),
            Cell::from(j.time_left.clone()).style(Style::default().fg(Color::White)),
            Cell::from(j.cpus.clone()).style(Style::default().fg(Color::DarkGray)),
            Cell::from(j.memory.clone()).style(Style::default().fg(Color::DarkGray)),
            Cell::from(j.nodes.clone()).style(Style::default().fg(Color::DarkGray)),
            Cell::from(j.reason.clone()).style(Style::default().fg(Color::DarkGray)),
        ])
    }).collect();

    let table = Table::new(rows, [
        Constraint::Length(10),  // Job ID      e.g. 45400853
        Constraint::Length(9),   // Partition   e.g. ncpu
        Constraint::Min(10),     // Name        expands to fill
        Constraint::Length(4),   // St          e.g. R
        Constraint::Length(10),  // Time Used   e.g. 9:49:39
        Constraint::Length(12),  // Time Left   e.g. 6-14:10:21
        Constraint::Length(5),   // CPUs        e.g. 64
        Constraint::Length(5),   // Mem         e.g. 32G
        Constraint::Length(6),   // Nodes       e.g. 1
        Constraint::Min(8),      // Node/Reason e.g. cn010
    ])
    .header(header)
    .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded)
        .border_style(Style::default().fg(SLURM_COLOR))
        .title(Span::styled(format!(" SLURM Jobs ({}) ", app.jobs.len()), Style::default().fg(Color::Gray))))
    .row_highlight_style(Style::default().bg(Color::Rgb(20, 50, 50)).add_modifier(Modifier::BOLD))
    .column_spacing(1);

    let mut state = TableState::default();
    frame.render_stateful_widget(table, area, &mut state);
}

// ---------------------------------------------------------------------------
// Status bar — flat hint bar, always the same
// ---------------------------------------------------------------------------
fn draw_status_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let content = if let Some(msg) = &app.status_message {
        Line::from(vec![Span::styled(
            format!(" ℹ  {msg}"),
            Style::default().fg(Color::Yellow),
        )])
    } else {
        // Overlay-specific hints override the default bar
        match &app.overlay {
            Overlay::NewSession | Overlay::Rename => hint_line(&[
                ("Enter", "confirm"), ("Esc", "cancel"),
            ]),
            Overlay::NewLayout => hint_line(&[
                ("↑/k ↓/j", "pick layout"), ("Enter", "confirm"), ("Esc", "cancel"),
            ]),
            Overlay::ConfirmKill => hint_line(&[
                ("y", "confirm kill"), ("n / Esc", "cancel"),
            ]),
            Overlay::Search => hint_line(&[
                ("type", "filter"), ("Enter", "select"), ("Esc", "clear"),
            ]),
            Overlay::None => {
                // Default: show all available actions
                let mut spans: Vec<Span> = vec![Span::raw(" ")];
                for (i, (key, desc)) in HINTS.iter().enumerate() {
                    spans.push(Span::styled(
                        format!(" {key} "),
                        Style::default().fg(Color::Black).bg(ACCENT).add_modifier(Modifier::BOLD),
                    ));
                    spans.push(Span::styled(
                        format!(" {desc} "),
                        Style::default().fg(Color::Gray),
                    ));
                    if i < HINTS.len() - 1 {
                        spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
                    }
                }
                Line::from(spans)
            }
        }
    };

    frame.render_widget(
        Paragraph::new(content)
            .block(Block::default().borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(ACCENT)))
            .alignment(Alignment::Left),
        area,
    );
}

fn hint_line(hints: &[(&str, &str)]) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = vec![Span::raw(" ")];
    for (i, (key, desc)) in hints.iter().enumerate() {
        spans.push(Span::styled(
            format!(" {key} "),
            Style::default().fg(Color::Black).bg(ACCENT).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!(" {desc} "),
            Style::default().fg(Color::Gray),
        ));
        if i < hints.len() - 1 {
            spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
        }
    }
    Line::from(spans)
}

// ---------------------------------------------------------------------------
// Overlays
// ---------------------------------------------------------------------------

fn draw_input_popup(frame: &mut Frame, title: &str, label: &str, input: &str) {
    let area = centered_rect(50, 9, frame.area());
    let block = Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .style(Style::default().bg(Color::Rgb(20, 15, 35)));
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)])
        .margin(1)
        .split(inner);

    frame.render_widget(
        Paragraph::new(Span::styled(label, Style::default().fg(Color::Gray))),
        rows[0],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            format!("{input}█"),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )),
        rows[1],
    );
}

fn draw_search_bar(frame: &mut Frame, app: &App) {
    let full = frame.area();
    let area = ratatui::layout::Rect {
        x: full.x + 1,
        y: full.height.saturating_sub(6),
        width: full.width.saturating_sub(2),
        height: 3,
    };
    let block = Block::default()
        .title(" Search ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .style(Style::default().bg(Color::Rgb(20, 15, 35)));
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(Span::styled(
            format!("/ {}█", app.input),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )),
        inner,
    );
}

fn draw_layout_picker(frame: &mut Frame, app: &App) {
    let height = (app.layouts.len() as u16 * 2 + 8).min(30);
    let area = centered_rect(60, height, frame.area());

    let block = Block::default()
        .title(" New Session — Choose Layout ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .style(Style::default().bg(Color::Rgb(20, 15, 35)));
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)])
        .margin(1)
        .split(inner);

    let items: Vec<ListItem> = app.layouts.iter().enumerate().map(|(i, l)| {
        let style = if i == app.layout_selected {
            Style::default().fg(Color::White).bg(Color::Rgb(50, 40, 80)).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        ListItem::new(Line::from(vec![
            Span::styled(format!(" {} ", l.name), style),
            Span::styled(format!("— {}", l.description), Style::default().fg(Color::DarkGray)),
        ]))
    }).collect();

    let mut list_state = ListState::default();
    list_state.select(Some(app.layout_selected));
    frame.render_stateful_widget(
        List::new(items).highlight_style(Style::default().bg(Color::Rgb(50, 40, 80))),
        rows[0],
        &mut list_state,
    );

    frame.render_widget(
        Paragraph::new(Span::styled("Session name:", Style::default().fg(Color::Gray))),
        rows[2],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            format!("{}█", app.input),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )),
        rows[3],
    );
}

fn draw_confirm_popup(frame: &mut Frame, app: &App) {
    let name = app.selected_session()
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "this session".to_string());

    let area = centered_rect(50, 9, frame.area());
    let block = Block::default()
        .title(" Kill Session ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Red))
        .style(Style::default().bg(Color::Rgb(30, 10, 10)));
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![
                Span::raw("  Kill session "),
                Span::styled(format!("'{name}'"), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::raw("?"),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  y ", Style::default().fg(Color::Black).bg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::styled(" confirm  ", Style::default().fg(Color::Gray)),
                Span::styled(" n ", Style::default().fg(Color::Black).bg(ACCENT).add_modifier(Modifier::BOLD)),
                Span::styled(" cancel", Style::default().fg(Color::Gray)),
            ]),
        ])
        .wrap(Wrap { trim: false }),
        inner,
    );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub fn fmt_duration(secs: u64) -> String {
    if secs < 60        { format!("{secs}s ago") }
    else if secs < 3600 { format!("{}m ago", secs / 60) }
    else if secs < 86400 { format!("{}h ago", secs / 3600) }
    else                { format!("{}d ago", secs / 86400) }
}

fn centered_rect(percent_x: u16, height: u16, area: ratatui::layout::Rect) -> ratatui::layout::Rect {
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Fill(1), Constraint::Length(height), Constraint::Fill(1)])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(v[1])[1]
}
