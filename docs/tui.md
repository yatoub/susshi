# TUI Guide

susshi provides an interactive terminal UI for browsing and connecting to servers.

## Navigation

| Key | Action |
| --- | --- |
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `Space` | Expand/collapse group |
| `Enter` | Connect / toggle group |
| `q` | Quit |

Mouse:

- Click: select item.
- Double-click: connect.

## Search and Filters

| Key | Action |
| --- | --- |
| `/` | Enter search mode |
| `Ctrl+U` | Clear search query |
| `Esc` | Cancel search |

Search behavior:

- Matches server name and hostname.
- Supports tag filters with `#tag` tokens.
- Multiple tags are AND-combined.
- Mixed queries are allowed (`api #prod`).
- `defaults.default_filter` pre-applies a startup filter.

## Connection Controls

| Key | Action |
| --- | --- |
| `Tab` | Cycle mode (Direct/Jump/Wallix) |
| `1` | Select Direct |
| `2` | Select Jump |
| `3` | Select Wallix |
| `v` | Toggle verbose SSH mode |
| `y` | Copy generated SSH command |

If the clipboard is unavailable (headless session, missing `wl-clipboard`, etc.), an overlay displays the command for manual copy. Dismiss with `Enter` or `Esc`.

### Mode-dependent details

The detail panel only shows the connection settings of the selected mode:

| Mode | Extra lines |
| --- | --- |
| Direct | none |
| Jump | `Jump:` — the jump host (`jump`) |
| Wallix | `Wallix:` — `bastion_user@bastion`, and `Authorization:` (see below) |

The Wallix `Authorization:` line follows the same resolution as SCP, tunnels, ad-hoc commands and diagnostics:

- declared authorizations (`wallix.authorization`, then `wallix.authorizations`), in the order they are tried;
- otherwise the name derived from the YAML structure, marked `(derived)`;
- otherwise `Wallix menu`: the selection is made in the bastion menu when connecting.

Nothing is shown when the selected mode is Jump or Wallix but the server has no jump host or bastion configured.

## Productivity Features

| Key | Action |
| --- | --- |
| `r` | Hot-reload all config files |
| `f` | Toggle favorite |
| `F` | Favorites-only view |
| `C` | Collapse all groups/environments/namespaces |
| `E` | Expand all groups/environments/namespaces |
| `Ctrl+Y` | Cycle Catppuccin theme variant (Latte → Frappe → Macchiato → Mocha → …) |
| `H` | Toggle recent-sort view |
| `h` | Toggle keyboard help overlay |
| `o` | Open group overview dashboard |
| `\|` | Pin/unpin server in split pane |

State is persisted in `~/.susshi_state.json` (favorites, expanded nodes, sort mode, last seen connection timestamps, tunnel overrides).

## Server Notes

Add a free-form `notes` field to any server in the YAML config:

```yaml
servers:
  - name: web-01
    host: 192.168.1.10
    notes: "staging — reboot window Thursday nights"
```

The note is displayed in the detail panel below the last-seen timestamp.

## Diagnostics and Commands

| Key | Action |
| --- | --- |
| `d` | Run quick diagnostics on the selected server |
| `x` | Run ad-hoc SSH command on the selected server |
| `p` | Prompt for SSH credential (passphrase if a key is configured, password otherwise) |
| `M` | Toggle mouse capture (disable to allow terminal text selection) |

Diagnostics include:

- Kernel, OS info, CPU model/core count, load average.
- RAM and disk usage bars.
- Extra filesystem checks from `probe_filesystems`.

Ad-hoc command output is displayed in the detail pane (up to 20 lines) with colored exit status.

Both `x` and `d` use the current connection mode (Direct, Jump or Wallix), with the server's `ssh_options`. They run non-interactively (`BatchMode=yes`): authentication must not require a prompt.

### Command history

When the ad-hoc command prompt (`x`) is open, use `↑` and `↓` to navigate previously run commands. The history is deduplicated (identical consecutive commands are not duplicated) and **persists across sessions** (saved in `~/.susshi_state.json`, capped at 100 entries).

## Group Overview Dashboard

Press `o` when a **group** or **environment** header is selected to open the overview dashboard.

susshi launches a parallel SSH probe on every server in the group and displays results in a live-updating overlay:

| Column | Description |
| --- | --- |
| ✓ / ✗ | Connection success or failure |
| Name | Server name |
| Host | SSH hostname |
| Status | Load average, RAM %, disk % on success; first line of error message on failure |

- Results arrive as threads complete; pending servers show `…`.
- Use `j`/`↓` and `k`/`↑` to scroll the list.
- Press `o` again or `Esc` to close.

The probe uses the current connection mode (Direct/Jump) selected in the tab bar.

## Split Pane

Press `|` on any **server** to pin it to a dedicated right panel.

The layout switches from a 2-column view (tree + details) to a 3-column view (tree + current details + pinned server). The pinned server panel shows name, host, user, port, group/environment, and connection mode.

Press `|` again on the same server (or on any non-server item) to unpin and return to the standard layout.

## Tunnels and SCP

| Key | Action |
| --- | --- |
| `T` | Open SSH tunnel manager |
| `s` | Open SCP transfer form |

See [tunnels.md](tunnels.md) and [scp.md](scp.md) for full details.

## Keyboard Help Overlay

Press `h` at any time to open an in-TUI reference of all keybindings. Press `h` again or `Esc` to close it.

## Error Overlay

Connection errors are shown as an in-TUI overlay.

Dismiss with `Enter`, `Esc`, or `q`.
