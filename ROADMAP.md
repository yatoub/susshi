# Roadmap

Planned features and long-term direction. Items are subject to change based on community feedback — open an [issue](https://github.com/yatoub/susshi/issues) to discuss priorities.

Full roadmap with version history: [yatoub.github.io/susshi/roadmap/](https://yatoub.github.io/susshi/roadmap/)

## Recently shipped

| Version | Highlights |
| --- | --- |
| v1.0 | Stable release: versioned `state.json`, legacy `mode: bastion` / `wallix_group` removed (see [Migrating to 1.0](configuration.md#migrating-to-10)) |
| v0.25 | Ad-hoc commands and probes through a Wallix bastion, SCP progress for Wallix transfers |
| v0.24 | SCP, tunnels and config-declared authorizations for Wallix |
| v0.23 | Production badge and opt-in connection confirmation |
| v0.22 | Include recursion limited to 2 levels, outdated include repo warnings |
| v0.21 | First-run wizard |
| v0.19–v0.20 | Nix flake, APT repository |
| v0.16–v0.17 | Hardened YAML deserialization, 86 % test coverage |
| v0.15 | Fuzzy search, manpage, Wallix custom headers, OpenSSH/CSV/Terraform/Nmap export, SSH agent socket per server, SCP, command history, notes field, live theme toggle |

## Stability from 1.0

From 1.0, susshi follows [Semantic Versioning](https://semver.org/): breaking changes to the following only happen in a new major version.

- the YAML inventory schema (keys, inheritance, `!include`, variable interpolation);
- the CLI flags and the output of `--list`/export formats;
- the persisted state file (`~/.susshi_state.json`), which is versioned and migrated automatically.

Platform support: Linux and macOS are fully supported. On Windows, the TUI and config parsing work, but interactive SSH (PTY) and Wallix are Unix-only; full Windows support is a long-term goal (below).

## Near-term

- [ ] **Homebrew tap** — `brew install yatoub/tap/susshi` for macOS users without cargo
- [x] **First-run wizard** — interactive prompt to create `~/.susshi.yml` on first launch
- [ ] **Connection log** — persist last-connected timestamp and last-run command per server
- [ ] **Multi-select operations** — apply tunnels, SCP, or exec-group to several servers at once

## Long-term

- [ ] **Config UI** — edit the inventory from within the TUI without touching YAML
- [ ] **Full Windows support** — PTY and Wallix support on Windows (interactive SSH is currently Unix-only)
- [ ] **Plugin system** — hook into the connection lifecycle with external binaries
- [ ] **susshi sync** — optional encrypted config sync via a self-hosted backend (privacy-preserving, no cloud dependency)
