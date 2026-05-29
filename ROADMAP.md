# Roadmap

Planned features and long-term direction. Items are subject to change based on community feedback — open an [issue](https://github.com/yatoub/susshi/issues) to discuss priorities.

Full roadmap with version history: [yatoub.github.io/susshi/roadmap/](https://yatoub.github.io/susshi/roadmap/)

## Recently shipped

| Version | Highlights |
| --- | --- |
| v0.17 | Improved README and documentation badges |
| v0.16 | Hardened YAML deserialization, 86 % test coverage |
| v0.15 | Fuzzy search, manpage, Wallix custom headers, OpenSSH/CSV/Terraform/Nmap export, SSH agent socket per server, SCP, command history, notes field, live theme toggle |

## Near-term

- [ ] **Homebrew tap** — `brew install yatoub/tap/susshi` for macOS users without cargo
- [ ] **First-run wizard** — interactive prompt to create `~/.susshi.yml` on first launch
- [ ] **Connection log** — persist last-connected timestamp and last-run command per server
- [ ] **Multi-select operations** — apply tunnels, SCP, or exec-group to several servers at once

## Long-term

- [ ] **Config UI** — edit the inventory from within the TUI without touching YAML
- [ ] **Full Windows TUI** — PTY support on Windows (currently config-only)
- [ ] **Plugin system** — hook into the connection lifecycle with external binaries
- [ ] **susshi sync** — optional encrypted config sync via a self-hosted backend (privacy-preserving, no cloud dependency)
