use anyhow::{Context, Result};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct Session {
    pub name: String,
    pub windows: usize,
    pub attached: bool,
    /// Seconds since last activity in this session.
    pub idle_secs: u64,
}

/// Idle threshold: sessions inactive for more than this are flagged.
pub const IDLE_WARN_SECS: u64 = 60 * 60; // 1 hour

/// List all tmux sessions by parsing `tmux list-sessions` output.
pub fn list_sessions() -> Result<Vec<Session>> {
    let output = Command::new("tmux")
        .args([
            "list-sessions",
            "-F",
            "#{session_name}|#{session_windows}|#{session_attached}|#{session_activity}",
        ])
        .output()
        .context("Failed to run tmux")?;

    if !output.status.success() {
        return Ok(vec![]);
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let sessions = stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(|line| {
            let parts: Vec<&str> = line.splitn(4, '|').collect();
            let activity_ts: u64 = parts.get(3).unwrap_or(&"0").parse().unwrap_or(0);
            let idle_secs = now.saturating_sub(activity_ts);
            Session {
                name: parts.first().unwrap_or(&"").to_string(),
                windows: parts.get(1).unwrap_or(&"0").parse().unwrap_or(0),
                attached: parts.get(2).unwrap_or(&"0") == &"1",
                idle_secs,
            }
        })
        .collect();

    Ok(sessions)
}

/// Create a new detached tmux session with the given name.
pub fn new_session(name: &str) -> Result<()> {
    let status = Command::new("tmux")
        .args(["new-session", "-d", "-s", name])
        .status()
        .context("Failed to run tmux")?;

    anyhow::ensure!(status.success(), "tmux new-session failed");
    Ok(())
}

/// Kill a tmux session by name.
pub fn kill_session(name: &str) -> Result<()> {
    let status = Command::new("tmux")
        .args(["kill-session", "-t", name])
        .status()
        .context("Failed to run tmux")?;

    anyhow::ensure!(status.success(), "tmux kill-session failed");
    Ok(())
}

/// Rename a tmux session.
pub fn rename_session(old: &str, new: &str) -> Result<()> {
    let status = Command::new("tmux")
        .args(["rename-session", "-t", old, new])
        .status()
        .context("Failed to run tmux")?;

    anyhow::ensure!(status.success(), "tmux rename-session failed");
    Ok(())
}

/// Add pane and window (tab) operations targeting a specific tmux session.
/// These are kept for future use when pane/tab management is re-introduced.

#[allow(dead_code)] pub fn list_windows(session: &str) -> Vec<(usize, String, bool)> {
    let output = Command::new("tmux")
        .args([
            "list-windows",
            "-t", session,
            "-F", "#{window_index}|#{window_name}|#{window_active}",
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
            let p: Vec<&str> = line.splitn(3, '|').collect();
            let idx: usize = p.first().unwrap_or(&"0").parse().unwrap_or(0);
            let name = p.get(1).unwrap_or(&"").to_string();
            let active = p.get(2).unwrap_or(&"0") == &"1";
            (idx, name, active)
        })
        .collect()
}

/// List panes in a session: returns (window_idx, pane_idx, active, dimensions) tuples.
#[allow(dead_code)]
pub fn list_panes(session: &str) -> Vec<(usize, usize, bool, String)> {
    let output = Command::new("tmux")
        .args([
            "list-panes",
            "-s",           // all panes across all windows
            "-t", session,
            "-F", "#{window_index}|#{pane_index}|#{pane_active}|#{pane_width}x#{pane_height}",
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
            let p: Vec<&str> = line.splitn(4, '|').collect();
            let win: usize = p.first().unwrap_or(&"0").parse().unwrap_or(0);
            let pane: usize = p.get(1).unwrap_or(&"0").parse().unwrap_or(0);
            let active = p.get(2).unwrap_or(&"0") == &"1";
            let dims = p.get(3).unwrap_or(&"").to_string();
            (win, pane, active, dims)
        })
        .collect()
}

// --- Tab (window) operations ---

#[allow(dead_code)]
pub fn new_window(session: &str) -> Result<()> {
    let status = Command::new("tmux")
        .args(["new-window", "-t", session])
        .status()
        .context("tmux new-window failed")?;
    anyhow::ensure!(status.success(), "tmux new-window failed");
    Ok(())
}

#[allow(dead_code)]
pub fn kill_window(session: &str, window_idx: usize) -> Result<()> {
    let target = format!("{session}:{window_idx}");
    let status = Command::new("tmux")
        .args(["kill-window", "-t", &target])
        .status()
        .context("tmux kill-window failed")?;
    anyhow::ensure!(status.success(), "tmux kill-window failed");
    Ok(())
}

#[allow(dead_code)]
pub fn rename_window(session: &str, window_idx: usize, name: &str) -> Result<()> {
    let target = format!("{session}:{window_idx}");
    let status = Command::new("tmux")
        .args(["rename-window", "-t", &target, name])
        .status()
        .context("tmux rename-window failed")?;
    anyhow::ensure!(status.success(), "tmux rename-window failed");
    Ok(())
}

#[allow(dead_code)]
pub fn select_window(session: &str, window_idx: usize) -> Result<()> {
    let target = format!("{session}:{window_idx}");
    let status = Command::new("tmux")
        .args(["select-window", "-t", &target])
        .status()
        .context("tmux select-window failed")?;
    anyhow::ensure!(status.success(), "tmux select-window failed");
    Ok(())
}

// --- Pane operations ---

#[allow(dead_code)]
pub fn split_pane_horizontal(session: &str) -> Result<()> {
    let status = Command::new("tmux")
        .args(["split-window", "-h", "-t", session])
        .status()
        .context("tmux split-window failed")?;
    anyhow::ensure!(status.success(), "tmux split-window -h failed");
    Ok(())
}

#[allow(dead_code)]
pub fn split_pane_vertical(session: &str) -> Result<()> {
    let status = Command::new("tmux")
        .args(["split-window", "-v", "-t", session])
        .status()
        .context("tmux split-window failed")?;
    anyhow::ensure!(status.success(), "tmux split-window -v failed");
    Ok(())
}

#[allow(dead_code)]
pub fn kill_pane(session: &str, window_idx: usize, pane_idx: usize) -> Result<()> {
    let target = format!("{session}:{window_idx}.{pane_idx}");
    let status = Command::new("tmux")
        .args(["kill-pane", "-t", &target])
        .status()
        .context("tmux kill-pane failed")?;
    anyhow::ensure!(status.success(), "tmux kill-pane failed");
    Ok(())
}

#[allow(dead_code)]
pub fn zoom_pane(session: &str, window_idx: usize, pane_idx: usize) -> Result<()> {
    let target = format!("{session}:{window_idx}.{pane_idx}");
    let status = Command::new("tmux")
        .args(["resize-pane", "-Z", "-t", &target])
        .status()
        .context("tmux resize-pane -Z failed")?;
    anyhow::ensure!(status.success(), "tmux zoom failed");
    Ok(())
}

#[allow(dead_code)]
pub fn select_pane(session: &str, window_idx: usize, pane_idx: usize) -> Result<()> {
    let target = format!("{session}:{window_idx}.{pane_idx}");
    let status = Command::new("tmux")
        .args(["select-pane", "-t", &target])
        .status()
        .context("tmux select-pane failed")?;
    anyhow::ensure!(status.success(), "tmux select-pane failed");
    Ok(())
}
/// Tears down the TUI before attaching and returns when the user detaches,
/// so the caller can reinitialize the TUI and resume.
pub fn attach_session(name: &str, conf_path: &std::path::Path) -> Result<()> {
    let conf = conf_path.to_str().unwrap_or("");

    // Source the config into the running tmux server so the hint bar
    // is applied even if the session was created before nexus started.
    let _ = Command::new("tmux")
        .args(["source-file", conf])
        .status();

    Command::new("tmux")
        .args(["attach-session", "-t", name])
        .status()
        .context("Failed to run tmux attach-session")?;
    Ok(())
}
