# Configuration Guide

susshi reads `~/.susshi.yml` by default. This page documents the full schema and inheritance model.

## Configuration Inheritance

Settings are resolved with this priority (highest last):

1. `defaults`
2. `group`
3. `environment`
4. `server`

### Field semantics by inheritance level

Not all fields follow the same rule. Three distinct semantics apply:

| Semantic | Fields | Behaviour |
|---|---|---|
| **OVERRIDE** | `user`, `ssh_key`, `ssh_cert`, `ssh_agent_sock`, `ssh_port`, `mode`, `ssh_options`, `wallix`, `jump` | Lower level replaces the parent value entirely |
| **REPLACE** | `tunnels` | Lower level replaces the parent list entirely; if absent the parent list is used |
| **UNION** | `tags`, `probe_filesystems` | Every level **appends** its values to the parent list (duplicates are removed) |

Fields only available in `defaults` (not overridable per group/env/server): `theme`, `keep_open`, `default_filter`, `use_system_ssh_config`, `control_master`, `agent_forwarding`, `control_path`, `control_persist`, `pre_connect_hook`, `post_disconnect_hook`, `hook_timeout_secs`.

### Field availability per level

| Field | `defaults` | `group` | `environment` | `server` |
|---|:---:|:---:|:---:|:---:|
| `user` | ✓ | ✓ | ✓ | ✓ |
| `ssh_key` | ✓ | ✓ | ✓ | ✓ |
| `ssh_cert` | ✓ | — | — | ✓ |
| `ssh_agent_sock` | ✓ | — | — | ✓ |
| `ssh_port` | ✓ | ✓ | ✓ | ✓ |
| `ssh_options` | ✓ | ✓ | ✓ | ✓ |
| `mode` | ✓ | ✓ | ✓ | ✓ |
| `wallix` block | ✓ | ✓ | ✓ | ✓ |
| `wallix_group` | — | ✓ | ✓ | ✓ |
| `jump` | ✓ | ✓ | ✓ | ✓ |
| `tunnels` | ✓ | ✓ | ✓ | ✓ |
| `tags` | ✓ | ✓ | ✓ | ✓ |
| `probe_filesystems` | ✓ | ✓ | ✓ | ✓ |
| `pre_connect_hook` | ✓ | — | — | ✓ |
| `post_disconnect_hook` | ✓ | — | — | ✓ |
| `hook_timeout_secs` | ✓ | — | — | — |
| `control_master` | ✓ | — | — | — |
| `agent_forwarding` | ✓ | — | — | — |
| `control_path` | ✓ | — | — | — |
| `control_persist` | ✓ | — | — | — |
| `use_system_ssh_config` | ✓ | — | — | — |
| `keep_open` | ✓ | — | — | — |
| `default_filter` | ✓ | — | — | — |
| `theme` | ✓ | — | — | — |
| `name` | — | ✓ | ✓ | ✓ |
| `host` | — | — | — | ✓ |
| `notes` | — | — | — | ✓ |

### Default values when absent

| Field | Default |
|---|---|
| `user` | `"root"` |
| `ssh_key` | `"~/.ssh/id_rsa"` |
| `ssh_port` | `22` |
| `mode` | `direct` |
| `hook_timeout_secs` | `5` |
| `control_master` | `false` |
| `agent_forwarding` | `false` |
| `control_path` | `"~/.ssh/ctl/%h_%p_%r"` |
| `control_persist` | `"10m"` |
| `keep_open` | `false` |
| `use_system_ssh_config` | `false` |
| `wallix.auto_select` | `true` |
| `wallix.fail_if_menu_match_error` | `true` |
| `wallix.selection_timeout_secs` | `8` |
| `wallix.direct` | `false` |
| `wallix.account` | `"default"` |
| `wallix.protocol` | `"SSH"` |

---

## `includes` (Multi-file Configuration)

Use `includes` to split configuration by team, perimeter, or environment:

```yaml
includes:
  - label: "DEV"
    path: "~/.susshi_dev.yml"
  - label: "QUALIF"
    path: "~/.susshi_qualif.yml"
    merge_defaults: true
```

Fields:

- `label`: displayed as a namespace header in the TUI.
- `path`: absolute path, `~`-expanded path, or a `https://` / `http://` URL.
- `merge_defaults` (optional, default: `false`): merge main-file defaults as base values for the included file.

### HTTPS includes

`path` can be a full HTTPS URL. susshi will fetch the file over TLS at startup:

```yaml
includes:
  - label: "SHARED"
    path: "https://inventory.example.com/team-servers.yml"
    merge_defaults: true
```

Behavior:

- The URL is fetched synchronously at startup (same point as file includes).
- HTTP errors and network failures are non-fatal — emitted as `LoadError` warnings.
- The downloaded content is validated with the same unknown-field checks as local files.
- Recursive includes inside a URL-fetched file use normal local-path resolution.

Behavior (all includes):

- Includes are resolved recursively.
- Circular includes are reported as non-blocking warnings.
- Missing/unreadable files are non-fatal warnings.
- Unknown YAML fields emit non-blocking `ValidationWarning` entries.

---

## `defaults`

Global values applied unless overridden. See the field availability table above for the full list.

### Jump block

`jump` is always a list (even for one host). The entire list is replaced by a child level that defines `jump`:

```yaml
jump:
  - host: "jump1.example.com"
    user: "jump-user"
  - host: "jump2.example.com"    # multi-hop: second hop
    user: "jump-user"
```

### Wallix block

```yaml
wallix:
  host: "bastion.example.com"
  user: "bastion-user"
  group: "devops-admins"
  account: "default"
  protocol: "SSH"
  auto_select: true
  fail_if_menu_match_error: true
  selection_timeout_secs: 8
  # direct: false          # set true to skip menu probe for direct-checkout targets
  # authorization: ""      # force exact Wallix authorization name (skips menu matching)
  # header_columns: ["ID", "Cible", "Autorisation"]  # override if bastion uses different column labels
```

`bastion` is accepted as a backward-compatible alias key.

Wallix-specific fields:

- `template` — login-string template. Default: `{target_user}@%n:SSH:{bastion_user}`.
- `direct` — skip the menu probe entirely. Use when the authorization is guaranteed.
- `authorization` — force the exact Wallix authorization name. Skips menu matching when set.
- `header_columns` — list of tokens used to detect the menu header row. Override if your bastion uses localized or custom column names.

### Tunnels

Tunnels use **REPLACE** semantics: a child level that defines `tunnels` replaces the parent list entirely. This allows per-environment or per-server tunnel sets without inheriting unwanted parent tunnels.

```yaml
tunnels:
  - local_port: 5432
    remote_host: 127.0.0.1
    remote_port: 5432
    label: "PostgreSQL"     # optional — auto-generated if absent
```

Not available in `wallix` mode.

### Tags

Tags use **UNION** semantics: every level (defaults → group → env → server) appends its tags to the accumulated set. Duplicates are removed (first occurrence wins).

```yaml
defaults:
  tags: [infra]
groups:
  - name: Prod
    tags: [prod]
    environments:
      - name: EU
        tags: [eu-west]
        servers:
          - name: web-01
            host: 10.0.0.1
            tags: [web]
# web-01 resolved tags: [infra, prod, eu-west, web]
```

Tags can be used to filter servers in the TUI (`/`) or in `--export-filter`.

---

## `_vars` and Interpolation

Define per-file variables:

```yaml
_vars:
  bastion: "bastion.prod.example.com"
  env: "prod"
```

Use placeholders in any string field:

```yaml
name: "api-{{ env }}"
host: "{{ bastion }}"
```

Rules:

- Scope is file-local (does not leak across includes).
- Undefined variables emit a non-blocking warning and leave the `{{ placeholder }}` intact.
- Built-in `{{ index }}` expands to the 1-based server position within each list.

---

## `groups`, `environments`, and `servers`

Top-level inventory is defined in `groups`.

A group can contain:

- `servers` directly (level 2: Group → Server), or
- nested `environments`, each with `servers` (level 3: Group → Environment → Server).

Any level may override defaults.

### Server-specific fields

- `name`, `host` — required
- `mode` — `direct`, `jump`, or `wallix`
- `tags` — string labels, merged with parent tags (UNION)
- `notes` — free-form description shown in the detail panel
- `ssh_key` — path to the private key (tilde expanded)
- `ssh_cert` — path to a signed SSH certificate passed alongside `ssh_key`
- `ssh_agent_sock` — path to a Unix socket for a dedicated SSH agent (Unix only)
- `tunnels` — predefined local-port-forwarding tunnels (REPLACE semantics)
- `pre_connect_hook`, `post_disconnect_hook` — server-level hook overrides

---

## Complete Example

See [../examples/full_config.yaml](../examples/full_config.yaml) for a complete annotated configuration.
