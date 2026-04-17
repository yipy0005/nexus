use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
pub enum JobState {
    Running,
    Pending,
    Completing,
    Failed,
    Cancelled,
    Timeout,
    Unknown(String),
}

impl JobState {
    pub fn from_str(s: &str) -> Self {
        match s.trim() {
            "RUNNING"    | "R"  => JobState::Running,
            "PENDING"    | "PD" => JobState::Pending,
            "COMPLETING" | "CG" => JobState::Completing,
            "FAILED"     | "F"  => JobState::Failed,
            "CANCELLED"  | "CA" => JobState::Cancelled,
            "TIMEOUT"    | "TO" => JobState::Timeout,
            other               => JobState::Unknown(other.to_string()),
        }
    }

    /// Short two-letter label matching squeue's %.2t format.
    pub fn short_label(&self) -> &str {
        match self {
            JobState::Running    => "R",
            JobState::Pending    => "PD",
            JobState::Completing => "CG",
            JobState::Failed     => "F",
            JobState::Cancelled  => "CA",
            JobState::Timeout    => "TO",
            JobState::Unknown(s) => s.as_str(),
        }
    }

    /// Full label for notifications.
    pub fn label(&self) -> &str {
        match self {
            JobState::Running    => "RUNNING",
            JobState::Pending    => "PENDING",
            JobState::Completing => "COMPLETING",
            JobState::Failed     => "FAILED",
            JobState::Cancelled  => "CANCELLED",
            JobState::Timeout    => "TIMEOUT",
            JobState::Unknown(s) => s.as_str(),
        }
    }

    pub fn color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            JobState::Running    => Color::Rgb(80, 200, 120),
            JobState::Pending    => Color::Yellow,
            JobState::Completing => Color::Cyan,
            JobState::Failed     => Color::Red,
            JobState::Cancelled  => Color::Rgb(150, 150, 150),
            JobState::Timeout    => Color::Rgb(220, 100, 40),
            JobState::Unknown(_) => Color::DarkGray,
        }
    }

    /// Returns true for terminal states (job is done, one-time notification).
    #[allow(dead_code)]
    pub fn is_terminal(&self) -> bool {
        matches!(self, JobState::Failed | JobState::Cancelled | JobState::Timeout)
    }
}

/// Matches your SQUEUE_FORMAT:
/// "%.8i %9P %30j %.8u %.2t %.12M %.12L %.5C %.7m %.4D %R %S"
#[derive(Debug, Clone)]
pub struct Job {
    pub id: String,
    pub partition: String,
    pub name: String,
    #[allow(dead_code)]
    pub user: String,
    pub state: JobState,
    pub time_used: String,
    pub time_left: String,
    pub cpus: String,
    pub memory: String,
    pub nodes: String,
    pub reason: String,
    pub start_time: String,
}

/// Returns true if SLURM's `squeue` is available on PATH.
pub fn is_available() -> bool {
    Command::new("squeue")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Fetch the current user's SLURM jobs using your exact squeue format,
/// parsed via a pipe-delimited internal format for reliability.
pub fn list_jobs() -> Vec<Job> {
    if !is_available() {
        return vec![];
    }

    let output = Command::new("squeue")
        .args([
            "--me",
            "--noheader",
            // Pipe-delimited version of your format for safe parsing
            "--format=%i|%P|%j|%u|%t|%M|%L|%C|%m|%D|%R|%S",
        ])
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return vec![],
    };

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(|line| {
            let p: Vec<&str> = line.splitn(12, '|').collect();
            let get = |i: usize| p.get(i).unwrap_or(&"").trim().to_string();
            Job {
                id:         get(0),
                partition:  get(1),
                name:       get(2),
                user:       get(3),
                state:      JobState::from_str(&get(4)),
                time_used:  get(5),
                time_left:  get(6),
                cpus:       get(7),
                memory:     get(8),
                nodes:      get(9),
                reason:     get(10),
                start_time: get(11),
            }
        })
        .collect()
}

/// Compare old and new job lists and return human-readable notifications
/// for any state changes (e.g. PENDING → RUNNING, RUNNING → FAILED).
pub fn detect_changes(old: &[Job], new: &[Job]) -> Vec<String> {
    let mut notes = Vec::new();

    for new_job in new {
        if let Some(old_job) = old.iter().find(|j| j.id == new_job.id) {
            if old_job.state != new_job.state {
                let icon = match new_job.state {
                    JobState::Running    => "⚡",
                    JobState::Completing => "✓",
                    JobState::Failed     => "✗",
                    JobState::Cancelled  => "✗",
                    JobState::Timeout    => "⏱",
                    _                    => "→",
                };
                notes.push(format!(
                    "{icon} job {} ({}) {} → {}",
                    new_job.id,
                    new_job.name,
                    old_job.state.label(),
                    new_job.state.label(),
                ));
            }
        } else {
            // New job appeared
            notes.push(format!(
                "+ job {} ({}) submitted [{}]",
                new_job.id, new_job.name, new_job.state.label()
            ));
        }
    }

    // Jobs that disappeared (completed/cancelled and no longer in queue)
    for old_job in old {
        if !new.iter().any(|j| j.id == old_job.id) {
            let icon = match old_job.state {
                JobState::Running | JobState::Completing => "✓",
                _ => "✗",
            };
            notes.push(format!(
                "{icon} job {} ({}) finished",
                old_job.id, old_job.name
            ));
        }
    }

    notes
}
