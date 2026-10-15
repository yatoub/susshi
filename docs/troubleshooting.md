# Troubleshooting

## In-TUI Diagnostics

Press `d` when a server is selected to run a quick diagnostic probe over SSH.

The overlay displays:

- Kernel version and OS info
- CPU model and core count
- Load average (1 min, 5 min, 15 min)
- RAM usage bar (used / total)
- Disk usage bars for `/` and any extra paths in `probe_filesystems`
- A ⚠ warning for any configured filesystem that is not mounted

The probe uses the current connection mode (Direct, Jump or Wallix). In Wallix mode it goes through the direct login, like SCP — see [wallix.md](wallix.md#commands-and-diagnostics).

## Verbose Mode

Press `v` in the TUI (or pass `--verbose` on the CLI) to enable verbose SSH output. The raw `ssh -v` output is shown in the detail pane after connecting, which exposes handshake details, key negotiation, and authentication steps.

## Persistent State (`~/.susshi_state.json`)

susshi writes state to `~/.susshi_state.json` after each session. The file contains:

| Key | Description |
| --- | --- |
| `favorites` | Set of server names marked as favorites |
| `expanded_items` | Set of group/environment names that are expanded in the tree |
| `sort_mode` | Current sort (alphabetical or recent-first) |
| `last_seen` | Map of `server_name → ISO timestamp` of last connection |
| `command_history` | Last 100 ad-hoc commands (deduplicated) |
| `tunnel_overrides` | User-created or user-edited tunnels per server |

To reset all state: `rm ~/.susshi_state.json`

To reset only tunnel overrides without losing favorites, edit the file and clear the `tunnel_overrides` key.

## Common Issues

### Connection fails immediately

1. Enable verbose mode (`v`) and retry — the SSH handshake error appears in the detail pane.
2. Check that `ssh_key` exists and is readable (`ls -la ~/.ssh/id_ed25519`).
3. If using jump mode, verify the jump host is reachable independently: `ssh jump-user@jump.example.com`.
4. If `use_system_ssh_config: true` is set, check `~/.ssh/config` for conflicting `Host` blocks.

### Config file not loading

Run `susshi --validate` to check all config files for syntax errors, unknown fields, and unresolved includes. Errors and warnings are printed with file names and line numbers.

### Wallix connection hangs or shows the raw menu

Declare the server authorization in `wallix.authorizations` so susshi connects directly without the menu. See [docs/wallix.md](wallix.md) for the full troubleshooting guide (`Cible invalide`, `signature algorithm not supported`, tunnels).

### SCP fails or a tunnel stops

The SCP result popup and the tunnel status show the message returned by `ssh`/`scp` (OpenSSH warnings removed). In Wallix mode the SCP error also shows the exact target used, to compare with a manual `scp`.

### Clipboard copy (`y`) shows a fallback overlay

The clipboard backend is unavailable. Install the required tool for your environment:

- X11: `xclip` or `xsel`
- Wayland: `wl-clipboard` (provides `wl-copy`)
- macOS: built-in (should work out of the box)

The fallback overlay displays the full SSH command — select and copy it manually.

### TUI looks garbled or colors are wrong

susshi requires a **truecolor terminal** (24-bit color). Verify with:

```bash
echo $COLORTERM   # should output "truecolor" or "24bit"
```

Common terminals with truecolor: Alacritty, kitty, WezTerm, iTerm2, modern GNOME Terminal.

### Mouse click does not work

Press `M` to toggle mouse capture. When mouse capture is disabled, terminal text selection works normally but TUI click events are not processed.
