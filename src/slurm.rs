use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
pub enum JobState {
    Running,
    Pending,
    Completing,
    Failed,
    Cancelled,
    Unknown(String),
}

impl JobState {
    fn from_str(s: &str) -> Self {
        match s.trim() {
            "RUNNING" | "R" => JobState::Running,
            "PENDING" | "PD" => JobState::Pending,
            "COMPLETING" | "CG" => JobState::Completing,
            "FAILED" | "F" => JobState::Failed,
            "CANCELLED" | "CA" => JobState::Cancelled,
            other => JobState::Unknown(other.to_string()),
        }
    }

    pub fn label(&self) -> &str {
        match self {
            JobState::Running => "RUNNING",
            JobState::Pending => "PENDING",
            JobState::Completing => "COMPLETING",
            JobState::Failed => "FAILED",
            JobState::Cancelled => "CANCELLED",
            JobState::Unknown(s) => s.as_str(),
        }
    }

    pub fn color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            JobState::Running => Color::Rgb(80, 200, 120),
            JobState::Pending => Color::Yellow,
            JobState::Completing => Color::Cyan,
            JobState::Failed => Color::Red,
            JobState::Cancelled => Color::Rgb(150, 150, 150),
            JobState::Unknown(_) => Color::DarkGray,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Job {
    pub id: String,
    pub name: String,
    pub state: JobState,
    pub partition: String,
    pub nodes: String,
    pub time_used: String,
    pub time_limit: String,
}

/// Returns true if SLURM's `squeue` command is available on PATH.
pub fn is_available() -> bool {
    Command::new("squeue")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Fetch the current user's SLURM jobs via `squeue`.
/// Returns an empty vec if SLURM is not available.
pub fn list_jobs() -> Vec<Job> {
    if !is_available() {
        return vec![];
    }

    let output = Command::new("squeue")
        .args([
            "--me",
            "--noheader",
            "--format=%i|%j|%T|%P|%N|%M|%l",
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
            let p: Vec<&str> = line.splitn(7, '|').collect();
            Job {
                id: p.first().unwrap_or(&"").to_string(),
                name: p.get(1).unwrap_or(&"").to_string(),
                state: JobState::from_str(p.get(2).unwrap_or(&"")),
                partition: p.get(3).unwrap_or(&"").to_string(),
                nodes: p.get(4).unwrap_or(&"").to_string(),
                time_used: p.get(5).unwrap_or(&"").to_string(),
                time_limit: p.get(6).unwrap_or(&"").to_string(),
            }
        })
        .collect()
}
