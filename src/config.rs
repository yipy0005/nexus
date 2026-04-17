use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

/// The bundled tmux config, compiled into the binary at build time.
const TMUX_CONF: &str = include_str!("nexus.tmux.conf");

/// Returns the path to the Nexus config directory (~/.config/nexus),
/// creating it if it doesn't exist.
pub fn config_dir() -> Result<PathBuf> {
    let dir = nexus_config_dir();
    fs::create_dir_all(&dir).context("Failed to create nexus config dir")?;
    Ok(dir)
}

fn nexus_config_dir() -> PathBuf {
    // Respect XDG_CONFIG_HOME if set, otherwise fall back to ~/.config/nexus
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("nexus")
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".config").join("nexus")
    }
}

/// Write the bundled tmux config to ~/.config/nexus/nexus.tmux.conf
/// and return its path. Called once at startup so attach can reference it.
///
/// Design goals:
///   - Two-row status: top = window list, bottom = keybinding hints
///   - Purple accent to match Nexus TUI
///   - Does NOT override the user's prefix -- uses default Ctrl+b
///   - Does NOT touch key bindings -- purely visual
pub fn write_tmux_conf() -> Result<PathBuf> {
    let dir = config_dir()?;
    let path = dir.join("nexus.tmux.conf");
    fs::write(&path, TMUX_CONF).context("Failed to write nexus.tmux.conf")?;
    Ok(path)
}
