---
description: Install susshi and connect to your first server in under 5 minutes.
---

# Getting Started

Get up and running with susshi in under 5 minutes.

## Prerequisites

- Linux, macOS, or WSL
- `ssh` client installed (`openssh-client` / `openssh`)

## Install

=== "Arch Linux"
    ```bash
    paru -S susshi-bin
    ```

=== "Linux (binary)"
    ```bash
    wget https://github.com/yatoub/susshi/releases/latest/download/susshi-linux-x86_64
    chmod +x susshi-linux-x86_64 && sudo mv susshi-linux-x86_64 /usr/local/bin/susshi
    ```

=== "Linux (musl / static)"
    ```bash
    wget https://github.com/yatoub/susshi/releases/latest/download/susshi-linux-x86_64-static
    chmod +x susshi-linux-x86_64-static && sudo mv susshi-linux-x86_64-static /usr/local/bin/susshi
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

=== "DEB / RPM"
    Download from the [releases page](https://github.com/yatoub/susshi/releases/latest).

Verify:

```bash
susshi --version
```

## Create your first inventory

Create `~/.susshi.yml`:

```yaml
defaults:
  user: "your-user"
  ssh_key: "~/.ssh/id_ed25519"

groups:
  - name: "My Servers"
    servers:
      - name: "web-01"
        host: "192.168.1.10"
        mode: "direct"
      - name: "db-01"
        host: "192.168.1.20"
        mode: "direct"
```

## Launch the TUI

```bash
susshi
```

You'll see your server list. Navigate with `j`/`k`, press `Enter` to connect, `/` to search.

## Essential keybindings

| Key | Action |
| --- | --- |
| `j` / `k` or `↓` / `↑` | Move down / up |
| `Enter` | Connect or toggle group |
| `/` | Fuzzy search |
| `Tab` | Switch mode (Direct / Jump / Wallix) |
| `f` | Toggle favorite |
| `F` | Show favorites only |
| `T` | Tunnel manager |
| `E` | Expand all groups |
| `h` | Keyboard help overlay |
| `q` | Quit |

## One-shot connection (no TUI)

```bash
susshi --direct your-user@192.168.1.10
```

## Validate your config

```bash
susshi --validate
```

Checks YAML syntax, variable resolution, and SSH key existence without opening a connection.

## Next steps

- [Configuration reference](configuration.md) — full schema, inheritance, variables, includes
- [TUI Guide](tui.md) — all keybindings, search syntax, tag filters, diagnostics
- [Use Cases](use-cases.md) — jump hosts, Wallix bastions, multi-environment setups
