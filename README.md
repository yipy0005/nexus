# Nexus

A Zellij-like TUI session manager built on top of tmux — lightweight, fast, and HPC-friendly.

**Created by Yew Mun Yip**

---

## Why Nexus?

Zellij is great, but its WebAssembly plugin runtime keeps sessions alive with non-trivial CPU and memory overhead — a problem on shared HPC login nodes monitored by tools like Arbiter2. Nexus gives you Zellij's ergonomics (persistent status bar, key hints, session manager) on top of tmux's near-zero idle footprint.

## Features

- Session manager TUI — list, create, rename, kill, attach
- Layout configs — spawn sessions with predefined pane arrangements (`~/.config/nexus/layouts.toml`)
- Session search — `/` to filter by name
- Idle session detection — flags sessions inactive for over an hour
- CPU quota monitoring — reads cgroup limits and blocks new sessions when usage is critical (Arbiter2-aware)
- SLURM job panel — toggle with `S` to see your queued/running jobs alongside sessions
- Bundled tmux hint bar — a persistent status bar inside every session showing the most useful tmux keybindings, so you never have to memorise them

## Keybindings

| Key | Action |
|-----|--------|
| `↑` / `k` | Navigate up |
| `↓` / `j` | Navigate down |
| `Enter` | Attach to selected session |
| `n` | New session |
| `l` | New session from layout |
| `r` | Rename session |
| `x` | Kill session |
| `/` | Search / filter sessions |
| `S` | Toggle SLURM jobs panel |
| `R` | Refresh |
| `q` / `Esc` | Quit |

Inside a tmux session, detach with `Ctrl+b d` to return to Nexus.

## Requirements

- Rust (edition 2024)
- tmux

## Build & Run

```bash
cargo build --release
./target/release/nexus
```

## Configuration

Nexus writes its config to `~/.config/nexus/` (respects `$XDG_CONFIG_HOME`):

- `nexus.tmux.conf` — bundled tmux config with the hint bar (auto-generated, do not edit)
- `layouts.toml` — define your own session layouts

### Example layout

```toml
[[layout]]
name = "ml"
description = "ML workflow: editor + training terminal + monitor"

  [[layout.pane]]
  # top pane — open your editor

  [[layout.pane]]
  split = "vertical"
  size_pct = 35
  command = "watch -n5 nvidia-smi"

  [[layout.pane]]
  split = "horizontal"
  size_pct = 40
  # training terminal
```

## License

MIT — © Yew Mun Yip
