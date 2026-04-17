// Nexus — a Zellij-like TUI session manager built on top of tmux.
// Created by Yew Mun Yip.
//
mod app;
mod config;
mod layouts;
mod resources;
mod slurm;
mod tmux;
mod ui;

use anyhow::Result;
use app::{App, Overlay};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{io, time::{Duration, Instant}};

fn main() -> Result<()> {
    if which_tmux().is_err() {
        eprintln!("nexus requires tmux to be installed and available in PATH.");
        std::process::exit(1);
    }

    let tmux_conf = config::write_tmux_conf()?;
    let config_dir = config::config_dir()?;
    layouts::write_default_if_missing(&config_dir)?;

    let mut app = App::new(config_dir)?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        run_app(&mut terminal, &mut app)?;

        match app.attach_target.take() {
            None => break,
            Some(target) => {
                disable_raw_mode()?;
                execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
                terminal.show_cursor()?;

                tmux::attach_session(&target, &tmux_conf)?;

                enable_raw_mode()?;
                execute!(terminal.backend_mut(), EnterAlternateScreen, EnableMouseCapture)?;
                terminal.clear()?;

                app.should_quit = false;
                app.refresh();
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    const CPU_REFRESH: Duration = Duration::from_secs(5);
    // Poll faster when panel is open (5s), slower in background (15s)
    const SLURM_REFRESH_ACTIVE: Duration = Duration::from_secs(5);
    const SLURM_REFRESH_BG: Duration = Duration::from_secs(15);
    // Clear job notifications after 8 seconds
    const NOTIF_TTL: Duration = Duration::from_secs(8);

    let mut last_cpu = Instant::now();
    let mut last_slurm = Instant::now();
    let mut notif_shown_at: Option<Instant> = None;

    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if app.should_quit {
            return Ok(());
        }

        // Clear notification after TTL
        if let Some(t) = notif_shown_at {
            if t.elapsed() >= NOTIF_TTL {
                app.slurm_notification = None;
                notif_shown_at = None;
            }
        }
        if app.slurm_notification.is_some() && notif_shown_at.is_none() {
            notif_shown_at = Some(Instant::now());
        }

        let slurm_interval = if app.show_slurm { SLURM_REFRESH_ACTIVE } else { SLURM_REFRESH_BG };
        let timeout = CPU_REFRESH
            .checked_sub(last_cpu.elapsed())
            .unwrap_or(Duration::ZERO)
            .min(slurm_interval.checked_sub(last_slurm.elapsed()).unwrap_or(Duration::ZERO));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                handle_key(app, key.code);
            }
        }

        if last_cpu.elapsed() >= CPU_REFRESH {
            app.refresh_cpu();
            last_cpu = Instant::now();
        }
        let slurm_due = last_slurm.elapsed() >= slurm_interval;
        if slurm_due && app.slurm_available {
            app.refresh_jobs();
            last_slurm = Instant::now();
        }
    }
}

// ---------------------------------------------------------------------------
// Single flat key handler — no modes
// ---------------------------------------------------------------------------
fn handle_key(app: &mut App, key: KeyCode) {
    // Overlays capture all input first
    match app.overlay {
        Overlay::NewSession | Overlay::Rename => {
            handle_text_input(app, key);
            return;
        }
        Overlay::NewLayout => {
            handle_layout_picker(app, key);
            return;
        }
        Overlay::ConfirmKill => {
            handle_confirm_kill(app, key);
            return;
        }
        Overlay::Search => {
            handle_search(app, key);
            return;
        }
        Overlay::None => {}
    }

    // Normal navigation and actions — always available
    app.clear_status();
    match key {
        // Navigation
        KeyCode::Up | KeyCode::Char('k')    => app.move_up(),
        KeyCode::Down | KeyCode::Char('j')  => app.move_down(),

        // Primary action
        KeyCode::Enter                       => app.attach_selected(),

        // Session management
        KeyCode::Char('n')                   => app.start_new_session(),
        KeyCode::Char('l')                   => app.start_new_layout(),
        KeyCode::Char('r')                   => app.start_rename(),
        KeyCode::Char('x')                   => app.start_kill(),

        // Search
        KeyCode::Char('/')                   => app.start_search(),

        // SLURM toggle
        KeyCode::Char('S')                   => app.toggle_slurm(),

        // Refresh
        KeyCode::Char('R')                   => app.refresh(),

        // Quit
        KeyCode::Char('q') | KeyCode::Esc   => app.should_quit = true,

        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Overlay handlers
// ---------------------------------------------------------------------------

fn handle_text_input(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Enter => match app.overlay {
            Overlay::NewSession => app.confirm_new_session(),
            Overlay::Rename     => app.confirm_rename(),
            _                   => {}
        },
        KeyCode::Esc       => app.close_overlay(),
        KeyCode::Backspace => { app.input.pop(); }
        KeyCode::Char(c)   => app.input.push(c),
        _ => {}
    }
}

fn handle_layout_picker(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Up | KeyCode::Char('k')   => app.layout_up(),
        KeyCode::Down | KeyCode::Char('j') => app.layout_down(),
        KeyCode::Enter                      => app.confirm_new_layout(),
        KeyCode::Esc                        => app.close_overlay(),
        KeyCode::Backspace                  => { app.input.pop(); }
        KeyCode::Char(c)                    => app.input.push(c),
        _ => {}
    }
}

fn handle_search(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Esc   => app.cancel_search(),
        KeyCode::Enter => app.overlay = Overlay::None,
        KeyCode::Backspace => {
            app.input.pop();
            if app.input.is_empty() { app.cancel_search(); }
        }
        KeyCode::Char(c) => {
            app.input.push(c);
            let indices = app.filtered_indices();
            if let Some(&first) = indices.first() {
                app.selected = first;
            }
        }
        _ => {}
    }
}

fn handle_confirm_kill(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Char('y') | KeyCode::Char('Y') => app.confirm_kill(),
        _                                        => app.close_overlay(),
    }
}

fn which_tmux() -> Result<()> {
    std::process::Command::new("which")
        .arg("tmux")
        .output()
        .map(|o| {
            if o.status.success() { Ok(()) }
            else { Err(anyhow::anyhow!("tmux not found")) }
        })?
}
