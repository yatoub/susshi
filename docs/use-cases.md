---
description: Real-world susshi configurations for jump hosts, Wallix bastions, multi-environment teams, tunnels, and SCP.
---

# Use Cases

Concrete susshi configurations for common infrastructure setups.

## Team managing a large server fleet

When your `~/.ssh/config` grows beyond a few dozen hosts, susshi's hierarchical inventory keeps things navigable.

```yaml
defaults:
  user: "ops"
  ssh_key: "~/.ssh/id_ed25519"

groups:
  - name: "Production"
    tags: ["prod"]
    servers:
      - name: "api-01"
        host: "10.0.1.10"
        tags: ["api", "critical"]
      - name: "api-02"
        host: "10.0.1.11"
        tags: ["api", "critical"]
      - name: "db-primary"
        host: "10.0.2.10"
        tags: ["db", "critical"]

  - name: "Staging"
    tags: ["staging"]
    servers:
      - name: "api-staging"
        host: "10.1.1.10"
        tags: ["api"]
```

Use `#prod` or `#api #critical` in the `/` search bar to filter instantly. Star frequently-used servers with `f` and toggle the favorites-only view with `F`.

---

## Multi-hop jump hosts

Connect through one or more bastion hosts without writing manual `ProxyJump` chains.

```yaml
defaults:
  user: "ops"
  ssh_key: "~/.ssh/id_ed25519"

groups:
  - name: "Internal (via bastion)"
    jump:
      - host: "bastion.example.com"
        user: "jump-user"
        ssh_key: "~/.ssh/jump_key"
    servers:
      - name: "internal-db"
        host: "172.16.0.10"
        mode: "jump"
      - name: "internal-api"
        host: "172.16.0.20"
        mode: "jump"
```

susshi builds the full `ProxyJump` chain automatically. Override `mode` on any individual server to bypass the jump for that host.

### Three-hop chain

```yaml
jump:
  - host: "public-bastion.example.com"
    user: "jump1"
  - host: "internal-bastion.corp"
    user: "jump2"
```

---

## Wallix bastion

For teams using a Wallix bastion, susshi handles the auth menu interactively.

```yaml
defaults:
  user: "ops@domain"

groups:
  - name: "Wallix Production"
    wallix:
      host: "wallix.corp.internal"
      port: 22
      user: "ops@domain"
      ssh_key: "~/.ssh/wallix_key"
    servers:
      - name: "prod-web-01"
        host: "prod-web-01"
        mode: "wallix"
      - name: "prod-db-01"
        host: "prod-db-01"
        mode: "wallix"
```

susshi connects to the bastion, presents its interactive target menu, and you select the final destination. See [Wallix bastion](wallix.md) for advanced configuration including `header_columns` customization.

---

## Multi-environment with variable inheritance

Avoid duplicating connection settings across staging, pre-prod, and production.

```yaml
defaults:
  user: "deploy"
  ssh_key: "~/.ssh/deploy_key"

groups:
  - name: "App Cluster"
    environments:
      - name: "staging"
        ssh_key: "~/.ssh/staging_key"
        servers:
          - name: "app-staging-01"
            host: "staging-01.internal"
      - name: "production"
        servers:
          - name: "app-prod-01"
            host: "prod-01.internal"
          - name: "app-prod-02"
            host: "prod-02.internal"
```

The `staging` environment overrides only `ssh_key`; all other settings inherit from `defaults`. Switch modes with `Tab` if a server supports both direct and jump access.

---

## SSH tunnels for development

Forward a remote service port to localhost without keeping a terminal open.

```yaml
defaults:
  user: "dev"

groups:
  - name: "Dev tunnels"
    servers:
      - name: "remote-postgres"
        host: "dev-db.example.com"
        mode: "direct"
        tunnels:
          - local_port: 5433
            remote_host: "localhost"
            remote_port: 5432
```

Open the tunnel manager with `T`, start/stop tunnels without disconnecting. Tunnel state persists between susshi sessions.

---

## SCP file transfers

Transfer files to/from any server in your inventory with `s` (SCP mode).

Useful for:

- Uploading config files to multiple servers
- Retrieving logs without `rsync` setup
- Copying build artifacts to a staging server

See [SCP Transfers](scp.md) for full usage.

---

## Ansible export

Once your inventory is in susshi, export it to Ansible format:

```bash
susshi --export ansible > inventory.yml
```

Other export formats: `--export terraform`, `--export nmap`, `--export csv`, `--export openssh`.

See [Import & Export](import-export.md) for details.

---

## Import from `~/.ssh/config`

Migrate your existing SSH config into susshi in one step:

```bash
susshi --import-ssh-config
```

susshi reads `~/.ssh/config`, groups hosts by their `Match` / `Host` blocks, and writes a `~/.susshi.yml` skeleton that you can refine.
