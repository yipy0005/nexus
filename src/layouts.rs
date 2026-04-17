use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// A named layout that spawns a tmux session with predefined panes/commands.
#[derive(Debug, Clone)]
pub struct Layout {
    pub name: String,
    pub description: String,
    pub panes: Vec<PaneConfig>,
}

#[derive(Debug, Clone)]
pub struct PaneConfig {
    /// Optional shell command to run in this pane on startup.
    pub command: Option<String>,
    /// Split direction relative to the previous pane.
    pub split: SplitDir,
    /// Size as a percentage (used for split sizing).
    pub size_pct: u8,
}

#[derive(Debug, Clone)]
pub enum SplitDir {
    /// First pane — no split needed.
    First,
    /// Split horizontally (side by side).
    Horizontal,
    /// Split vertically (top/bottom).
    Vertical,
}

/// Load layouts from ~/.config/nexus/layouts.toml.
/// Returns built-in defaults if the file doesn't exist.
pub fn load(config_dir: &PathBuf) -> Vec<Layout> {
    let path = config_dir.join("layouts.toml");
    if path.exists() {
        match parse_toml(&path) {
            Ok(layouts) if !layouts.is_empty() => return layouts,
            _ => {}
        }
    }
    builtin_layouts()
}

/// Write a starter layouts.toml to the config dir if it doesn't exist yet.
pub fn write_default_if_missing(config_dir: &PathBuf) -> Result<()> {
    let path = config_dir.join("layouts.toml");
    if !path.exists() {
        fs::write(&path, DEFAULT_LAYOUTS_TOML)
            .context("Failed to write default layouts.toml")?;
    }
    Ok(())
}

/// Spawn a new tmux session using the given layout.
pub fn spawn(session_name: &str, layout: &Layout) -> Result<()> {
    // Create the session with the first pane
    let first_cmd = layout.panes.first()
        .and_then(|p| p.command.as_deref())
        .unwrap_or("");

    let mut args = vec!["new-session", "-d", "-s", session_name];
    if !first_cmd.is_empty() {
        args.push(first_cmd);
    }

    let status = Command::new("tmux")
        .args(&args)
        .status()
        .context("Failed to create tmux session")?;
    anyhow::ensure!(status.success(), "tmux new-session failed");

    // Add remaining panes
    for pane in layout.panes.iter().skip(1) {
        let split_flag = match pane.split {
            SplitDir::Horizontal => "-h",
            SplitDir::Vertical | SplitDir::First => "-v",
        };

        let size = format!("{}", pane.size_pct);
        let mut split_args = vec![
            "split-window",
            split_flag,
            "-t",
            session_name,
            "-p",
            &size,
        ];

        let cmd_str;
        if let Some(cmd) = &pane.command {
            cmd_str = cmd.clone();
            split_args.push(&cmd_str);
        }

        Command::new("tmux")
            .args(&split_args)
            .status()
            .context("Failed to split pane")?;
    }

    // Focus the first pane
    Command::new("tmux")
        .args(["select-pane", "-t", &format!("{session_name}:0.0")])
        .status()
        .ok();

    Ok(())
}

// ---------------------------------------------------------------------------
// TOML parsing (manual — avoids adding serde dependency for now)
// ---------------------------------------------------------------------------

fn parse_toml(path: &PathBuf) -> Result<Vec<Layout>> {
    let content = fs::read_to_string(path).context("Failed to read layouts.toml")?;
    let mut layouts = Vec::new();
    let mut current: Option<Layout> = None;

    for line in content.lines() {
        let line = line.trim();

        if line.starts_with("[[layout]]") {
            if let Some(l) = current.take() {
                layouts.push(l);
            }
            current = Some(Layout {
                name: String::new(),
                description: String::new(),
                panes: Vec::new(),
            });
        } else if line.starts_with("[[layout.pane]]") {
            if let Some(ref mut l) = current {
                l.panes.push(PaneConfig {
                    command: None,
                    split: if l.panes.is_empty() {
                        SplitDir::First
                    } else {
                        SplitDir::Vertical
                    },
                    size_pct: 50,
                });
            }
        } else if let Some(ref mut layout) = current {
            if let Some((key, val)) = parse_kv(line) {
                match key {
                    "name" => layout.name = val.to_string(),
                    "description" => layout.description = val.to_string(),
                    "command" => {
                        if let Some(pane) = layout.panes.last_mut() {
                            pane.command = Some(val.to_string());
                        }
                    }
                    "split" => {
                        if let Some(pane) = layout.panes.last_mut() {
                            pane.split = match val {
                                "horizontal" => SplitDir::Horizontal,
                                _ => SplitDir::Vertical,
                            };
                        }
                    }
                    "size_pct" => {
                        if let Some(pane) = layout.panes.last_mut() {
                            pane.size_pct = val.parse().unwrap_or(50);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    if let Some(l) = current {
        layouts.push(l);
    }

    Ok(layouts)
}

/// Parse a `key = "value"` or `key = 42` line.
fn parse_kv(line: &str) -> Option<(&str, &str)> {
    let (key, rest) = line.split_once('=')?;
    let key = key.trim();
    let val = rest.trim().trim_matches('"');
    Some((key, val))
}

// ---------------------------------------------------------------------------
// Built-in layouts (used when no layouts.toml exists)
// ---------------------------------------------------------------------------

fn builtin_layouts() -> Vec<Layout> {
    vec![
        Layout {
            name: "single".to_string(),
            description: "One pane, plain shell".to_string(),
            panes: vec![PaneConfig {
                command: None,
                split: SplitDir::First,
                size_pct: 100,
            }],
        },
        Layout {
            name: "dev".to_string(),
            description: "Editor top, terminal bottom".to_string(),
            panes: vec![
                PaneConfig {
                    command: None,
                    split: SplitDir::First,
                    size_pct: 100,
                },
                PaneConfig {
                    command: None,
                    split: SplitDir::Vertical,
                    size_pct: 30,
                },
            ],
        },
        Layout {
            name: "monitor".to_string(),
            description: "Shell left, htop right".to_string(),
            panes: vec![
                PaneConfig {
                    command: None,
                    split: SplitDir::First,
                    size_pct: 100,
                },
                PaneConfig {
                    command: Some("htop".to_string()),
                    split: SplitDir::Horizontal,
                    size_pct: 40,
                },
            ],
        },
    ]
}

const DEFAULT_LAYOUTS_TOML: &str = r#"# Nexus layout definitions
# Each [[layout]] block defines a named session layout.
# Panes are created in order; the first pane is always the initial window.

[[layout]]
name = "single"
description = "One pane, plain shell"

  [[layout.pane]]
  # No command = plain shell

[[layout]]
name = "dev"
description = "Editor top, terminal bottom"

  [[layout.pane]]
  # top pane — open your editor here

  [[layout.pane]]
  split = "vertical"
  size_pct = 30
  # bottom pane — terminal

[[layout]]
name = "monitor"
description = "Shell left, htop right"

  [[layout.pane]]
  # left pane

  [[layout.pane]]
  split = "horizontal"
  size_pct = 40
  command = "htop"
"#;
