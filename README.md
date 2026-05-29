# 🍣 susshi

[![CI](https://img.shields.io/github/actions/workflow/status/yatoub/susshi/ci.yml?branch=master&style=flat-square)](https://github.com/yatoub/susshi/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/susshi?style=flat-square)](https://crates.io/crates/susshi)
[![Crates downloads](https://img.shields.io/crates/d/susshi?style=flat-square)](https://crates.io/crates/susshi)
[![Documentation](https://img.shields.io/badge/docs-yatoub.github.io%2Fsusshi-blue?style=flat-square)](https://yatoub.github.io/susshi/)

[![MSRV](https://img.shields.io/badge/MSRV-1.88-blue?style=flat-square)](Cargo.toml)
[![Maintenance](https://img.shields.io/badge/maintenance-active-brightgreen?style=flat-square)](https://github.com/yatoub/susshi)
[![Security Audit](https://img.shields.io/github/actions/workflow/status/yatoub/susshi/ci.yml?label=Security%20Audit&logo=github&style=flat-square)](https://github.com/yatoub/susshi/actions/workflows/ci.yml)
[![Top language](https://img.shields.io/github/languages/top/yatoub/susshi?style=flat-square)](https://github.com/yatoub/susshi)
[![codecov](https://codecov.io/gh/yatoub/susshi/branch/master/graph/badge.svg)](https://codecov.io/gh/yatoub/susshi)

**susshi** is a terminal-based SSH connection manager written in Rust — hierarchical inventory, Catppuccin-themed TUI, jump hosts, Wallix bastions, tunnels, and SCP in one place.

[![asciicast](https://asciinema.org/a/1154653.svg)](https://asciinema.org/a/1154653)

## 🧐 Why Susshi?

Why use `susshi` rather than plain `ssh` / a single `~/.ssh/config`?

Managing dozens or hundreds of hosts with a single `ssh_config` file quickly becomes fragile and inefficient: no centralized visibility, no interactive interface, limited grouping, and poor ergonomics for common tasks (favorites, tunnels, SCP, bastions).

`susshi` addresses these gaps with clear benefits:

- **Ergonomics & speed**: fuzzy search, keyboard-driven navigation, help overlays and one-key connect — no editing and re-running commands.
- **Infrastructure visibility**: hierarchical grouping, favorites, tags and filtered views that make the inventory understandable at a glance.
- **Bastion & multi-hop workflows**: automatic orchestration of jump hops and Wallix integration — compose complex connection paths without repetitive `ssh_config` entries.
- **Inventory management & reusability**: variable inheritance, environment profiles, YAML includes and exports to Ansible/Terraform — maintain templates instead of copies.
- **Built-in operations**: tunnels, integrated SCP, pre/post-connect hooks and `--validate` checks to avoid human errors.
- **Security & privacy**: local-first configuration, no cloud dependency, and automated audits in CI (cargo-deny).

In short: `susshi` preserves compatibility with the SSH ecosystem while adding ergonomics, visibility and primitives to automate and secure your connections at scale.

---

### 🥊 How it compares to the alternatives

| Feature | **Susshi** 🍣 | **Standard SSH Config** 📄 | **Termius / Warp** ☁️ | **ClusterSSH** 🕸️ |
| :--- | :--- | :--- | :--- | :--- |
| **Interface** | Modern, interactive TUI | Raw text / `grep` | Heavy GUI / Electron | Outdated X11 windows |
| **Speed** | Instant (Native Rust) | Instant | Resource-heavy | Laggy on modern systems |
| **Privacy** | 100% Local & Open Source | 100% Local | Cloud-first / Proprietary | Local |
| **Workflow** | Fuzzy-finding & Tags | Manual typing | Mouse-driven | Multi-window chaos |
| **Terminal Agnostic** | Works in **any** terminal | Works in any terminal | Forces their own app | Requires X11 forwarding |

---

### ✨ Key Benefits

* **⚡ Blazing Fast & Lightweight:** Built in Rust with a `Ratatui` frontend. It compiles to a tiny native binary (< 10MB) that starts instantly and consumes virtually zero RAM.
* **🔒 Privacy & Security First:** Your infrastructure layout is sensitive data. Susshi is local-first; it requires no login, no cloud synchronization, and features zero telemetry. Your data stays where it belongs: on your machine.
* **🧠 Keyboard-Driven Ergonomics:** Keep your hands on the home row. Instantly filter through hundreds of servers using fuzzy search, navigate with Arrow/Vim keys, and connect with a single press of `Enter`.
* **🧩 Fits Your Existing Workflow:** Susshi doesn't try to replace your ecosystem. It works perfectly inside `tmux`, `screen`, or your favorite terminal emulator (Alacritty, Kitty, iTerm2, WezTerm), acting as a dedicated Unix utility that does one thing right.

## Installation

```bash
# Linux x86_64
wget https://github.com/yatoub/susshi/releases/latest/download/susshi-linux-amd64
chmod +x susshi-linux-amd64 && sudo mv susshi-linux-amd64 /usr/local/bin/susshi

# Linux x86_64 (musl fallback)
wget https://github.com/yatoub/susshi/releases/latest/download/susshi-linux-amd64-musl

# macOS Apple Silicon
wget https://github.com/yatoub/susshi/releases/latest/download/susshi-macos-arm64

# macOS Intel
wget https://github.com/yatoub/susshi/releases/latest/download/susshi-macos-amd64

# Arch Linux
paru -S susshi-bin
```

For DEB/RPM packages see the [releases page](https://github.com/yatoub/susshi/releases/latest).

> **Windows:** partial support — TUI and config parsing work, interactive SSH (PTY) and Wallix are Unix-only.

## Quick Start

Create `~/.susshi.yml`:

```yaml
defaults:
  user: "ops-user"
  ssh_key: "~/.ssh/id_ed25519"
  theme: mocha  # latte | frappe | macchiato | mocha

groups:
  - name: "Production"
    servers:
      - name: "api-01"
        host: "198.51.100.10"
        mode: "direct"
```

```bash
susshi          # open TUI
susshi --direct ops-user@198.51.100.10   # one-shot connection
```

Full annotated example: [examples/full_config.yaml](examples/full_config.yaml)

## Essential Keybindings

| Key | Action |
| --- | --- |
| `j` / `k` | Move down / up |
| `Enter` | Connect or toggle group |
| `/` | Search |
| `Tab` | Switch mode (Direct / Jump / Wallix) |
| `f` / `F` | Favorite toggle / favorites-only view |
| `T` | Tunnel manager |
| `h` | Keyboard help overlay |
| `q` | Quit |

## Documentation

| Guide | Description |
| --- | --- |
| [docs/configuration.md](docs/configuration.md) | Full config schema, inheritance model, includes, `_vars` |
| [docs/tui.md](docs/tui.md) | TUI navigation, search, keybindings, diagnostics |
| [docs/cli.md](docs/cli.md) | One-shot connection, `--validate`, `--exec-group` |
| [docs/import-export.md](docs/import-export.md) | Import from `~/.ssh/config`, export to Ansible/Terraform/Nmap/CSV |
| [docs/wallix.md](docs/wallix.md) | Wallix bastion configuration and troubleshooting |
| [docs/tunnels.md](docs/tunnels.md) | SSH tunnel configuration and TUI manager |
| [docs/scp.md](docs/scp.md) | In-TUI SCP file transfers |
| [docs/hooks.md](docs/hooks.md) | Pre/post-connect shell hooks |
| [docs/ssh-advanced.md](docs/ssh-advanced.md) | Certificates, agent sockets, ControlMaster, agent forwarding |
| [docs/troubleshooting.md](docs/troubleshooting.md) | Diagnostics, common issues, state file |

## Contributing

Contributions are welcome. Please open a Pull Request — see [CONTRIBUTING.md](CONTRIBUTING.md) for conventions.

## License

This project is licensed under the MIT License.
