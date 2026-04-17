use crate::layouts::{self, Layout};
use crate::resources::{self, CpuStatus};
use crate::slurm::{self, Job};
use crate::tmux::{self, Session};
use anyhow::Result;
use std::path::PathBuf;

/// Transient overlays — popups that capture input temporarily.
#[derive(Debug, Clone, PartialEq)]
pub enum Overlay {
    None,
    NewSession,
    NewLayout,
    Rename,
    ConfirmKill,
    Search,
}

pub struct App {
    // Sessions
    pub sessions: Vec<Session>,
    pub selected: usize,

    // Overlays / input
    pub overlay: Overlay,
    pub input: String,

    // Feedback
    pub status_message: Option<String>,

    // Control
    pub should_quit: bool,
    pub attach_target: Option<String>,

    // Resources
    pub cpu: CpuStatus,

    // Layouts
    pub layouts: Vec<Layout>,
    pub layout_selected: usize,

    // SLURM
    pub jobs: Vec<Job>,
    pub slurm_available: bool,
    /// Whether the SLURM panel is currently visible.
    pub show_slurm: bool,

    // Last attached session (for window/pane info display)
    pub last_session: Option<String>,

    pub _config_dir: PathBuf,
}

impl App {
    pub fn new(config_dir: PathBuf) -> Result<Self> {
        let sessions = tmux::list_sessions()?;
        let cpu = resources::sample();
        let layouts = layouts::load(&config_dir);
        let slurm_available = slurm::is_available();
        let jobs = if slurm_available { slurm::list_jobs() } else { vec![] };

        Ok(Self {
            selected: 0,
            sessions,
            overlay: Overlay::None,
            input: String::new(),
            status_message: None,
            should_quit: false,
            attach_target: None,
            cpu,
            layouts,
            layout_selected: 0,
            jobs,
            slurm_available,
            show_slurm: false,
            last_session: None,
            _config_dir: config_dir,
        })
    }

    // -----------------------------------------------------------------------
    // Refresh
    // -----------------------------------------------------------------------

    pub fn refresh(&mut self) {
        match tmux::list_sessions() {
            Ok(sessions) => {
                self.sessions = sessions;
                if self.selected >= self.sessions.len() {
                    self.selected = self.sessions.len().saturating_sub(1);
                }
            }
            Err(e) => self.set_status(format!("Error: {e}")),
        }
        self.cpu = resources::sample();
    }

    pub fn refresh_cpu(&mut self) {
        self.cpu = resources::sample();
    }

    pub fn refresh_jobs(&mut self) {
        if self.slurm_available {
            self.jobs = slurm::list_jobs();
        }
    }

    pub fn toggle_slurm(&mut self) {
        self.show_slurm = !self.show_slurm;
        if self.show_slurm {
            self.refresh_jobs();
        }
    }

    // -----------------------------------------------------------------------
    // Filtered session list
    // -----------------------------------------------------------------------

    pub fn filtered_indices(&self) -> Vec<usize> {
        if self.overlay != Overlay::Search || self.input.is_empty() {
            return (0..self.sessions.len()).collect();
        }
        let query = self.input.to_lowercase();
        self.sessions
            .iter()
            .enumerate()
            .filter(|(_, s)| s.name.to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn selected_session(&self) -> Option<&Session> {
        self.sessions.get(self.selected)
    }

    // -----------------------------------------------------------------------
    // Navigation
    // -----------------------------------------------------------------------

    pub fn move_up(&mut self) {
        let indices = self.filtered_indices();
        if indices.is_empty() { return; }
        if let Some(pos) = indices.iter().position(|&i| i == self.selected) {
            if pos > 0 { self.selected = indices[pos - 1]; }
        } else {
            self.selected = *indices.first().unwrap();
        }
    }

    pub fn move_down(&mut self) {
        let indices = self.filtered_indices();
        if indices.is_empty() { return; }
        if let Some(pos) = indices.iter().position(|&i| i == self.selected) {
            if pos + 1 < indices.len() { self.selected = indices[pos + 1]; }
        } else {
            self.selected = *indices.first().unwrap();
        }
    }

    pub fn layout_up(&mut self) {
        if self.layout_selected > 0 { self.layout_selected -= 1; }
    }

    pub fn layout_down(&mut self) {
        if !self.layouts.is_empty() && self.layout_selected < self.layouts.len() - 1 {
            self.layout_selected += 1;
        }
    }

    // -----------------------------------------------------------------------
    // Status
    // -----------------------------------------------------------------------

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some(msg.into());
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
    }

    pub fn close_overlay(&mut self) {
        self.overlay = Overlay::None;
        self.input.clear();
        self.clear_status();
    }

    // -----------------------------------------------------------------------
    // Actions
    // -----------------------------------------------------------------------

    pub fn start_search(&mut self) {
        self.overlay = Overlay::Search;
        self.input.clear();
        self.clear_status();
    }

    pub fn cancel_search(&mut self) {
        self.overlay = Overlay::None;
        self.input.clear();
    }

    pub fn start_new_session(&mut self) {
        self.overlay = Overlay::NewSession;
        self.input.clear();
        self.clear_status();
    }

    pub fn start_new_layout(&mut self) {
        self.overlay = Overlay::NewLayout;
        self.input.clear();
        self.layout_selected = 0;
        self.clear_status();
    }

    pub fn start_rename(&mut self) {
        if let Some(s) = self.selected_session() {
            self.input = s.name.clone();
            self.overlay = Overlay::Rename;
            self.clear_status();
        }
    }

    pub fn start_kill(&mut self) {
        if self.selected_session().is_some() {
            self.overlay = Overlay::ConfirmKill;
            self.clear_status();
        }
    }

    pub fn confirm_new_session(&mut self) {
        let name = self.input.trim().to_string();
        self.overlay = Overlay::None;
        self.input.clear();

        if name.is_empty() {
            self.set_status("Session name cannot be empty.");
            return;
        }

        self.cpu = resources::sample();
        if resources::would_exceed_on_new_session(&self.cpu) {
            self.set_status(format!(
                "Blocked: CPU at {:.0}% of quota ({:.1} cores). \
                 Kill idle sessions to avoid Arbiter2 penalties.",
                self.cpu.usage_pct, self.cpu.quota_cores
            ));
            return;
        }

        match tmux::new_session(&name) {
            Ok(_) => {
                self.refresh();
                if let Some(idx) = self.sessions.iter().position(|s| s.name == name) {
                    self.selected = idx;
                }
                self.set_status(format!("Created session '{name}'."));
            }
            Err(e) => self.set_status(format!("Error: {e}")),
        }
    }

    pub fn confirm_new_layout(&mut self) {
        let name = self.input.trim().to_string();
        self.overlay = Overlay::None;
        self.input.clear();

        if name.is_empty() {
            self.set_status("Session name cannot be empty.");
            return;
        }

        self.cpu = resources::sample();
        if resources::would_exceed_on_new_session(&self.cpu) {
            self.set_status(format!(
                "Blocked: CPU at {:.0}% of quota ({:.1} cores). \
                 Kill idle sessions to avoid Arbiter2 penalties.",
                self.cpu.usage_pct, self.cpu.quota_cores
            ));
            return;
        }

        let layout = match self.layouts.get(self.layout_selected) {
            Some(l) => l.clone(),
            None => { self.set_status("No layout selected."); return; }
        };

        match layouts::spawn(&name, &layout) {
            Ok(_) => {
                self.refresh();
                if let Some(idx) = self.sessions.iter().position(|s| s.name == name) {
                    self.selected = idx;
                }
                self.set_status(format!("Created '{}' with layout '{}'.", name, layout.name));
            }
            Err(e) => self.set_status(format!("Error: {e}")),
        }
    }

    pub fn confirm_rename(&mut self) {
        let new_name = self.input.trim().to_string();
        self.overlay = Overlay::None;
        self.input.clear();

        if new_name.is_empty() {
            self.set_status("Name cannot be empty.");
            return;
        }

        if let Some(old_name) = self.selected_session().map(|s| s.name.clone()) {
            match tmux::rename_session(&old_name, &new_name) {
                Ok(_) => {
                    self.refresh();
                    if let Some(idx) = self.sessions.iter().position(|s| s.name == new_name) {
                        self.selected = idx;
                    }
                    self.set_status(format!("Renamed to '{new_name}'."));
                }
                Err(e) => self.set_status(format!("Error: {e}")),
            }
        }
    }

    pub fn confirm_kill(&mut self) {
        self.overlay = Overlay::None;
        if let Some(name) = self.selected_session().map(|s| s.name.clone()) {
            match tmux::kill_session(&name) {
                Ok(_) => {
                    self.refresh();
                    self.set_status(format!("Killed session '{name}'."));
                }
                Err(e) => self.set_status(format!("Error: {e}")),
            }
        }
    }

    pub fn attach_selected(&mut self) {
        if let Some(s) = self.selected_session() {
            let name = s.name.clone();
            self.last_session = Some(name.clone());
            self.attach_target = Some(name);
            self.should_quit = true;
        }
    }
}
