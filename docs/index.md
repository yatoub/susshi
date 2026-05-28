---
description: susshi is a terminal SSH manager with YAML inventories, multi-hop jump hosts, Wallix bastion, tunnels, SCP, and Catppuccin TUI — written in Rust.
---

# susshi 🍣

**susshi** is a terminal-based SSH connection manager written in Rust — hierarchical YAML inventory, Catppuccin-themed TUI, jump hosts, Wallix bastions, tunnels, and SCP in one place.

[Get Started](getting-started.md){ .md-button .md-button--primary }
[View on GitHub](https://github.com/yatoub/susshi){ .md-button }

---

<script async id="asciicast-1154653" src="https://asciinema.org/a/1154653.js"></script>

---

## Why susshi?

Managing dozens or hundreds of hosts with a single `~/.ssh/config` quickly becomes fragile: no centralized visibility, no interactive interface, limited grouping, and poor ergonomics for day-to-day tasks.

`susshi` addresses these gaps:

- **Ergonomics & speed** — fuzzy search, keyboard-driven navigation, one-key connect. No editing files and re-running commands.
- **Infrastructure visibility** — hierarchical groups, favorites, tag filters, interactive inventory at a glance.
- **Bastion & multi-hop workflows** — automatic orchestration of jump hops and Wallix integration without repetitive `ssh_config` entries.
- **Inventory management** — variable inheritance, environment profiles, YAML includes, and exports to Ansible/Terraform/Nmap/CSV.
- **Built-in operations** — tunnels, SCP, pre/post-connect hooks, `--validate` checks.
- **Security & privacy** — local-first, no cloud dependency, no telemetry.

## How it compares

| Feature | **susshi** 🍣 | **SSH Config** 📄 | **Termius / Warp** ☁️ | **ClusterSSH** 🕸️ |
| :--- | :--- | :--- | :--- | :--- |
| Interface | Modern interactive TUI | Raw text / `grep` | Heavy GUI / Electron | Outdated X11 windows |
| Speed | Instant (native Rust) | Instant | Resource-heavy | Laggy on modern systems |
| Privacy | 100% local & open source | 100% local | Cloud-first / proprietary | Local |
| Workflow | Fuzzy search & tags | Manual typing | Mouse-driven | Multi-window |
| Terminal | Any terminal emulator | Any terminal | Forces their own app | Requires X11 |

## Quick install

=== "Arch Linux"
    ```bash
    paru -S susshi-bin
    ```

=== "Linux (binary)"
    ```bash
    wget https://github.com/yatoub/susshi/releases/latest/download/susshi-linux-x86_64
    chmod +x susshi-linux-x86_64 && sudo mv susshi-linux-x86_64 /usr/local/bin/susshi
    ```

=== "macOS (Apple Silicon)"
    ```bash
    wget https://github.com/yatoub/susshi/releases/latest/download/susshi-macos-apple-silicon
    chmod +x susshi-macos-apple-silicon && sudo mv susshi-macos-apple-silicon /usr/local/bin/susshi
    ```

=== "macOS (Intel)"
    ```bash
    wget https://github.com/yatoub/susshi/releases/latest/download/susshi-macos-intel
    chmod +x susshi-macos-intel && sudo mv susshi-macos-intel /usr/local/bin/susshi
    ```

=== "Cargo"
    ```bash
    cargo install susshi
    ```

For DEB/RPM packages see the [releases page](https://github.com/yatoub/susshi/releases/latest).

## What's included

| Feature | Description |
| --- | --- |
| Fuzzy search | Filter hundreds of servers instantly with `#tag` and text queries |
| Jump hosts | Multi-hop connections without manual `ProxyJump` chains |
| Wallix bastion | Full Wallix auth menu integration |
| SSH tunnels | Persistent tunnels with in-TUI manager |
| SCP transfers | File transfers without leaving the terminal |
| Import / Export | Import from `~/.ssh/config`, export to Ansible, Terraform, Nmap, CSV |
| Hooks | Shell scripts triggered before connect / after disconnect |
| Themes | Live Catppuccin theme toggle (latte / frappe / macchiato / mocha) |

## Next steps

- [Getting Started](getting-started.md) — install, first config, first connection
- [Configuration](configuration.md) — full YAML schema and inheritance model
- [Use Cases](use-cases.md) — real-world setups for jump hosts, Wallix, multi-env
- [Roadmap](roadmap.md) — what's coming next
