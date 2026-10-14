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

## Automatic SSH User Identity

When `mode: wallix` is active, susshi builds the SSH `User` field as:

`<wallix_user>@<target_fqdn>:SSH[:<ENV_GROUP>]:<wallix_user>`

Where:

- `<target_fqdn>` comes from the selected server host.
- `<ENV_GROUP>` is included only when a deterministic authorization group is resolved.
- If resolution is ambiguous, susshi avoids generating a potentially wrong authorization silently.

## Selection Strategy

When `mode: wallix` is active, susshi computes candidates before selecting an ID in the Wallix menu:

- Target candidates: FQDN, short host, and structure-derived aliases.
- Group candidates: configured `wallix.group` plus structure-derived variants.
- Prefixed authorizations are supported (for example, `ST-ANSIBLE_devops-admins` matches `devops-admins`).

## Targeted Fallback Popup

When automatic resolution fails (no match or ambiguity), susshi opens a focused popup instead of exposing the full global menu flow.

- Candidate entries are pre-filtered against the current target server.
- Search starts from the target hostname to reduce noise.
- The selected Wallix entry ID is cached for the current session.
- During nominal flow, the global pseudo-TTY menu output stays hidden.

## Pagination and Fallback

- Paginated menus are scanned automatically (`page X/Y`, then `n`).
- If Wallix asks for a secondary prompt like `Adresse cible`, susshi auto-fills the configured server host.
- If no reliable match is found, susshi falls back to manual in-session selection.

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

Resolution order (interactive SSH and SCP):

1. `wallix.authorization` — exact name, always first.
2. `wallix.authorizations` — declared candidates, in order.
3. Last resort: detection — the name derived from the YAML structure (`<ENV>-<GROUP>_<group>`), then the Wallix menu.

When exactly **one** authorization is known from the configuration, susshi connects directly with `ssh -A -l user@target+SSH+AUTHORIZATION+bastion_user bastion`: no menu probe, no PTY relay. Agent forwarding (`-A`) lets the bastion authenticate on the target with your SSH agent. With several candidates, the Wallix menu picks among them (in order). SCP tries each candidate in order and moves to the next one when Wallix answers `Cible invalide`.

An entry containing `{group}` is ignored when no `wallix.group` is resolved.

A known authorization also enables [SSH tunnels](tunnels.md) through the bastion (direct login, background shell session).

## Forcing a Specific Authorization

When a target has multiple Wallix authorizations (different groups), set `wallix.authorization` to the exact authorization name shown in the Wallix menu:

```yaml
groups:
  - name: "Secured Perimeter"
    wallix:
      group: "ces3s-admins"         # short form — used for menu scoring
    servers:
      - name: "anscore02"
        host: "anscore02.internal.example"
        mode: "wallix"
        wallix:
          authorization: "STI-ANSCORE_ces3s-admins"  # exact name from the Wallix menu
          direct: true
```

`authorization` takes priority over `authorizations` and `group`. It is passed verbatim in the direct login string (`user@target+SSH+AUTHORIZATION+bastion_user`), so the connection opens immediately without going through the menu.

**When to use:** When `auto_select` resolves the wrong entry, or when a server has multiple authorizations and you want to pin the right one without going through the menu.

## Direct Connection Mode

Set `wallix.direct: true` to bypass the menu probe entirely and connect in zero delay.

```yaml
defaults:
  wallix:
    host: "bastion.example.com"
    user: "bastion-user"
    direct: true          # skip menu probe — Wallix connects directly
```

**When to use:** When Wallix is configured to connect the target directly without presenting a selection menu, or when `wallix.authorization` is set and Wallix will always resolve to a single entry. Without `direct: true`, susshi still detects direct connections automatically — it just takes a few extra seconds.

## Behavior Controls

- `auto_select`: enable automatic selection attempts.
- `fail_if_menu_match_error`: keep trying (including pagination) before falling back.
- `selection_timeout_secs`: menu parsing timeout budget.
- `authorization`: exact Wallix authorization name — bypasses group matching and pins the entry.
- `direct`: skip the menu probe and connect immediately (zero-delay).
- `header_columns`: list of tokens used to detect and skip the menu header line. Default: `["ID", "Cible", "Autorisation"]`. Override if your bastion shows column headers in a different language:

```yaml
defaults:
  wallix:
    header_columns: ["Num", "Target", "Authorization"]
```

## Troubleshooting

### No matching authorization

- Verify `wallix.group` at defaults/group/environment/server level.
- Ensure the target host in config matches the Wallix entry target naming.
- If still unresolved, use the popup and keep the selected entry for the current session.

### Ambiguous authorization

- Multiple authorizations can match the same server target.
- susshi will not guess silently and opens the targeted popup.
- Set a more specific `wallix.group` on the server to make resolution deterministic.

## Notes

- Wallix mode can be inherited from defaults/group/environment.
- ControlMaster multiplexing is silently disabled in Wallix mode.
