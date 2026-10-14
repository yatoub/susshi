# Wallix Guide

This page describes how susshi behaves in `wallix` mode.

## Basic Configuration

```yaml
defaults:
  wallix:
    host: "bastion.example.com"
    user: "bastion-user"
    group: "devops-admins"
    account: "default"
    protocol: "SSH"
    auto_select: true
    fail_if_menu_match_error: true
    selection_timeout_secs: 8
```

Enable Wallix on a server:

```yaml
- name: "internal-nas"
  host: "198.51.100.200"
  mode: "wallix"
```

## Declaring Authorizations (recommended)

Declare the Wallix authorizations in the configuration so susshi never has to guess. `wallix.authorizations` is a list inherited like any other Wallix field (defaults → group → environment → server, the most specific level wins). `{group}` is replaced by the resolved `wallix.group` suffix:

```yaml
defaults:
  wallix:
    host: "ssh.bastion.example"
    user: "jdoe"
    group: "ces3s-admins"            # suffix shared by every authorization

groups:
  - name: "COLIBRIS"
    environments:
      - name: "PR"
        wallix:
          authorizations:
            - "PR-COLIBRIS_{group}"  # → PR-COLIBRIS_ces3s-admins
      - name: "PP - BaS"
        wallix:
          authorizations:            # several candidates, in order of preference
            - "PP-COLIBRIS_{group}"
            - "PP-BAS_{group}"
```

Resolution order (interactive SSH, SCP and tunnels):

1. `wallix.authorization` — exact name, always first (`{group}` is not replaced there).
2. `wallix.authorizations` — declared candidates, in order.
3. Last resort: detection — the name derived from the YAML structure (`<ENV>-<GROUP>_<group>`), then the Wallix menu.

An entry containing `{group}` is ignored when no `wallix.group` is resolved.

> **Tip:** the authorization names are listed in the Wallix menu (`Autorisation` column) when you connect to the bastion without a target.

## How susshi Connects

### Direct login (authorization known)

When exactly **one** authorization is known from the configuration, susshi connects directly:

```bash
ssh -A -l "user@target+SSH+AUTHORIZATION+bastion_user" bastion
```

There is no menu probe and no PTY relay: `ssh` drives the terminal itself. Agent forwarding (`-A`) lets the bastion authenticate on the target with your SSH agent, so your key must be loaded in the agent (`ssh-add -l`).

### Menu flow (last resort)

Without a single known authorization (none, or several candidates), susshi goes through the Wallix menu: it opens the bastion session in a PTY, parses the menu, selects the matching entry, then relays the session to your terminal. Declared candidates are tried first when matching the menu, then the detected names.

The relay puts your terminal in raw mode and forwards window resizes, so Tab completion, arrow keys, Ctrl+R/Ctrl+C and full-screen tools (`vim`, `less`, `top`) behave as in a plain `ssh` session.

## Group Inheritance and Override

`wallix.group` follows the same inheritance model as other susshi settings.

- If `server.wallix.group` is set, it has priority.
- Otherwise susshi uses the inherited group from environment/group/defaults.
- If no group can be resolved, susshi switches to targeted fallback selection.

Example:

```yaml
defaults:
  wallix:
    host: "bastion.example.com"
    user: "wallix-user"
    group: "devops-admins"

groups:
  - name: "Databases"
    servers:
      - name: "db-ops-01"
        host: "db-ops-01.internal.example"
        mode: "wallix"
        wallix:
          group: "qa-admins"
```

## Menu Flow Details

### SSH user identity

In the menu flow, susshi builds the SSH `User` field as:

`<wallix_user>@<target_fqdn>:SSH[:<ENV_GROUP>]:<wallix_user>`

Where:

- `<target_fqdn>` comes from the selected server host.
- `<ENV_GROUP>` is included only when a deterministic authorization group is resolved.
- If resolution is ambiguous, susshi avoids generating a potentially wrong authorization silently.

### Selection strategy

susshi computes candidates before selecting an ID in the Wallix menu:

- Target candidates: FQDN, short host, and structure-derived aliases.
- Authorization candidates: declared `authorization` / `authorizations` first, then `wallix.group` and structure-derived variants.
- Prefixed authorizations are supported (for example, `ST-ANSIBLE_devops-admins` matches `devops-admins`).

### Targeted fallback popup

When automatic resolution fails (no match or ambiguity), susshi opens a focused popup instead of exposing the full global menu flow.

- Candidate entries are pre-filtered against the current target server.
- Search starts from the target hostname to reduce noise.
- The selected Wallix entry ID is cached for the current session.
- During nominal flow, the global pseudo-TTY menu output stays hidden.

### Pagination and fallback

- Paginated menus are scanned automatically (`page X/Y`, then `n`).
- If Wallix asks for a secondary prompt like `Adresse cible`, susshi auto-fills the configured server host.
- If no reliable match is found, susshi falls back to manual in-session selection.

### Direct connection mode (`direct`)

`wallix.direct: true` skips the menu probe in the menu flow, for targets that Wallix always connects directly without presenting a selection menu. It has **no effect** when an authorization is known from the configuration — the direct login is used anyway.

```yaml
defaults:
  wallix:
    host: "bastion.example.com"
    user: "bastion-user"
    direct: true          # skip menu probe — Wallix connects directly
```

## Forcing a Specific Authorization

When a server needs an authorization that does not follow the shared `{group}` suffix (for example a `…_crtech-admins` authorization on a single server), set `wallix.authorization` to the exact name shown in the Wallix menu:

```yaml
groups:
  - name: "Secured Perimeter"
    servers:
      - name: "anscore02"
        host: "anscore02.internal.example"
        mode: "wallix"
        wallix:
          authorization: "STI-ANSCORE_crtech-admins"  # exact name from the Wallix menu
```

`authorization` takes priority over `authorizations` and `group`. It is passed verbatim in the direct login string, so the connection opens immediately without going through the menu. When the name follows the shared suffix, prefer `authorizations: ["STI-ANSCORE_{group}"]`.

## SCP and Tunnels

- **SCP** ([scp.md](scp.md)) goes through the system `scp` binary with the direct login: `scp -A … user@target+SSH+AUTHORIZATION+bastion_user@bastion:path`. Declared authorizations are tried in order, moving to the next one while Wallix answers `Cible invalide`; the derived name is the last resort.
- **Tunnels** ([tunnels.md](tunnels.md)) require a known authorization (a background tunnel cannot answer the menu). susshi runs `ssh -A -T -L …` and keeps a shell session open on the target, because Wallix closes the connection when port forwarding is requested outside an open session. Port forwarding is not necessarily enabled on every environment: ask the bastion administrator if needed.

## Behavior Controls

- `authorizations`: candidate authorization names, in order; `{group}` is replaced by `group`.
- `authorization`: exact Wallix authorization name — highest priority, pins the entry.
- `auto_select`: enable automatic selection attempts (menu flow).
- `fail_if_menu_match_error`: keep trying (including pagination) before falling back (menu flow).
- `selection_timeout_secs`: menu parsing timeout budget (menu flow).
- `direct`: skip the menu probe (menu flow only).
- `header_columns`: list of tokens used to detect and skip the menu header line. Default: `["ID", "Cible", "Autorisation"]`. Override if your bastion shows column headers in a different language:

```yaml
defaults:
  wallix:
    header_columns: ["Num", "Target", "Authorization"]
```

## Troubleshooting

### `Cible invalide`

Wallix rejects the target/authorization pair in the login string.

- Check the authorization name against the `Autorisation` column of the Wallix menu.
- Check that `host` is the name Wallix knows for the target (usually the FQDN).
- For SCP, the error lists every target that was tried.

### `signature algorithm not supported`

The bastion signs with `ssh-rsa` (SHA-1), refused by default by recent OpenSSH. Allow it for susshi connections:

```yaml
defaults:
  ssh_options:
    - "PubkeyAcceptedKeyTypes=+ssh-rsa"
```

### Tunnel stops right after starting

- The authorization may not allow port forwarding: ask the bastion administrator.
- The tunnel status shows the message returned by `ssh` / the bastion.

### No matching authorization (menu flow)

- Declare the authorization in `wallix.authorizations` (see above).
- Otherwise verify `wallix.group` at defaults/group/environment/server level and ensure the target host in config matches the Wallix entry target naming.
- If still unresolved, use the popup and keep the selected entry for the current session.

### Ambiguous authorization (menu flow)

- Multiple authorizations can match the same server target.
- susshi will not guess silently and opens the targeted popup.
- Declare the right one in `wallix.authorizations`, or set `wallix.authorization` on the server.

## Notes

- Wallix mode can be inherited from defaults/group/environment.
- ControlMaster multiplexing is silently disabled in Wallix mode.
- Wallix mode is Unix-only.
