# SSH Tunnels

susshi supports SSH local-port-forwarding tunnels, configured per-server and managed interactively from the TUI.

## Configuration

Define tunnels in the YAML config at any level (defaults, group, environment, server). A `tunnels` list at a lower level **replaces** the one inherited from above.

```yaml
servers:
  - name: db-01
    host: 198.51.100.11
    tunnels:
      - local_port: 5432
        remote_host: 127.0.0.1
        remote_port: 5432
        label: "PostgreSQL"
      - local_port: 6379
        remote_host: 127.0.0.1
        remote_port: 6379
        label: "Redis"
```

Each entry produces: `ssh -L <local_port>:<remote_host>:<remote_port> -N …`

Fields:

| Field | Required | Description |
| --- | --- | --- |
| `local_port` | Yes | Port bound on `localhost` |
| `remote_host` | Yes | Host as seen from the remote server (usually `127.0.0.1`) |
| `remote_port` | Yes | Port on the remote side |
| `label` | No | Display name shown in the TUI tunnel list |

> **Wallix mode:** tunnels require the server's Wallix authorization to be known from the configuration (`wallix.authorization`, or a single entry in `wallix.authorizations`, see [Wallix Guide](wallix.md#declaring-authorizations-recommended)); without it, Wallix shows its selection menu, which a background tunnel cannot answer. susshi runs `ssh -A -T -L <local_port>:<server FQDN>:<remote_port> -l user@target+SSH+AUTHORIZATION+bastion_user bastion` and keeps the remote shell session open in the background: Wallix closes the connection when port forwarding is requested outside an open session on the target, so `-N` cannot be used. A local `remote_host` (`127.0.0.1`, `localhost`, `::1`) is replaced by the server FQDN, as documented by the bastion. If the bastion refuses the tunnel, its message is shown in the tunnel status. Port forwarding through the bastion is not necessarily enabled by default on every environment: ask the bastion administrator to enable it if needed (the TUI tunnel list shows this reminder in Wallix mode).

## `keep_open`

Set `keep_open: true` at any config level to automatically reopen the TUI after a connection closes. Useful when you want to run a tunnel, disconnect, and immediately pick another server.

```yaml
defaults:
  keep_open: false  # default
```

## TUI Tunnel Manager

Press `T` in the TUI to open the tunnel manager overlay.

Actions:

| Key | Action |
| --- | --- |
| `Enter` | Start or stop the selected tunnel |
| `a` | Add a new tunnel (user override) |
| `e` | Edit the selected tunnel |
| `D` | Delete the selected tunnel |
| `Esc` | Close the tunnel manager |

Active tunnels show a live status badge next to the server name in the tree.

## User Overrides

Tunnels added or edited through the TUI are stored as **user overrides** in `~/.susshi_state.json`. Overrides persist across restarts and are merged with config-defined tunnels at runtime. Deleting an override reverts the server to its config-defined tunnels.
