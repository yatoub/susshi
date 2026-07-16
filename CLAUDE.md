# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this project is

**Susshi** is a terminal-based SSH connection manager (TUI) written in Rust. It provides a Catppuccin-themed interface for organizing servers with hierarchical groups/environments, multiple connection modes (direct, jump hops, Wallix bastion), tunnels, SCP transfers, and ad-hoc command running.

## Commands

```bash
cargo build --release          # release build (optimized for size)
cargo test --verbose           # run tests
cargo fmt --all -- --check     # check formatting
cargo clippy --all-targets --all-features -- -D warnings  # strict lint
cargo build --features openssl-vendored  # musl targets needing vendored OpenSSL
cargo cov                      # coverage summary excluding UI/main/i18n (requires cargo-llvm-cov)
cargo cov --html               # HTML coverage report in target/llvm-cov/html/
```

CI also tests these targets: `x86_64-unknown-linux-musl`, `x86_64-pc-windows-msvc`, `x86_64-apple-darwin`, `aarch64-apple-darwin`.

## Architecture

### Entry point and state machine

[main.rs](src/main.rs) handles CLI parsing (clap), terminal initialization via crossterm, and the main event loop. [app.rs](src/app.rs) + [app/](src/app/) define the application state machine split across 11 submodules:

- `core_state.rs` — `AppMode` enum and central state struct
- `lifecycle.rs` — startup/shutdown, config reload
- `search.rs`, `visible_items.rs`, `selection.rs` — filtering and list navigation
- `favorites.rs`, `expansion_state.rs` — persistent UI state
- `command.rs`, `wallix_state.rs`, `tunnel_state.rs`, `scp_state.rs` — mode-specific state

### Configuration

[config.rs](src/config.rs) + [config/](src/config/) parse YAML inventory files, resolve `!include` directives, apply variable interpolation, and resolve the final connection mode (direct / single-hop / multi-hop jump / Wallix) from potentially inherited settings across group → environment → server, split across submodules:

- `types.rs` — struct/enum definitions (`Config`, `Group`, `Environment`, `Server`, `ResolvedServer`, etc.)
- `resolve.rs` — inheritance resolution (`impl Config`, `resolve_entries`, `resolve_server`, `merge_bastion`/`merge_jump`)
- `interpolate.rs` — variable interpolation (`interpolate`, `undefined_vars`, `merge_default_structs`)
- `validate.rs` — YAML structural validation (`validate_yaml`, `fetch_url` for remote includes)

### UI layer

[ui/mod.rs](src/ui/mod.rs) is the main Ratatui draw function. [ui/overlays.rs](src/ui/overlays.rs) and [ui/panels.rs](src/ui/panels.rs) handle modal dialogs and list panels. [ui/theme.rs](src/ui/theme.rs) wires the catppuccin crate into Ratatui styles.

### SSH operations

- [ssh/client.rs](src/ssh/client.rs) + [ssh/client/](src/ssh/client/) — builds the `ssh` command, forkpty on Unix, Wallix auth flow, split into `args.rs` (argument building), `connect.rs` (direct/jump connection entry points), `wallix_pty.rs` (interactive Wallix PTY auth flow)
- [ssh/sftp.rs](src/ssh/sftp.rs) — SCP transfers using ssh2
- [ssh/tunnel.rs](src/ssh/tunnel.rs) — SSH tunnel lifecycle
- [wallix/](src/wallix/) — Wallix bastion auth menu parsing and target selection, split into `target.rs` (target/group string building), `menu.rs` (menu text parsing), `select.rs` (entry selection logic)
- [hooks.rs](src/hooks.rs) — pre-connect / post-disconnect shell hooks

### Supporting modules

- [import.rs](src/import.rs) — parses `~/.ssh/config` into susshi YAML
- [export/](src/export/) — inventory export to Ansible, CSV, Nmap, OpenSSH config, and Terraform formats
- [probe.rs](src/probe.rs) — diagnostics (disk, SSH keys)
- [i18n.rs](src/i18n.rs) — localization
- [state.rs](src/state.rs) — persistent state (favorites, tunnel overrides) written to disk

## Development environment

- Shell: **fish** (not bash) — use fish syntax in any shell snippets
- Workflow: **TDD** — write the test first, then the implementation

## Key constraints

**Cross-platform:** Unix-specific code must be gated with `#[cfg(unix)]`. The `nix` crate (pty/process/term features) is only in `[target.'cfg(unix)'.dependencies]`. New platform-specific code must compile on Windows/macOS.

**No `unwrap()` in production code.** Use `anyhow` / `thiserror` for error handling.

**Release process is fully automated via release-plz** — never manually edit `Cargo.toml` version, `CHANGELOG.md`, or push version tags. The CI pipeline (release-plz.yml → release.yml → aur-publish.yml) handles versioning, crates.io publish, binary/DEB/RPM packaging, and AUR PKGBUILD updates.

**Never push directly to master.** All changes must go through a PR.

**Conventional commits are required:** `feat:`, `fix:`, `refactor:`, `docs:`, `test:`, `chore:`, `perf:` — these drive automatic changelog generation and version bumping.

- `feat:` on a 0.x crate → **minor bump** (e.g. 0.15.x → 0.16.0) — enforced by `features_always_increment_minor = true` in `release-plz.toml`.
- `ci:`, `test:`, `chore:`, `docs:` commits are excluded from the changelog and do not trigger a release PR alone (`release_commits` guard in `release-plz.toml`).

## CI pipeline

```
ci.yml      preflight → lint → test → coverage
                                    → cross-platform smoke builds (5 targets)
            audit, megalinter (independent)

release-plz.yml   push to master → open/update release PR or cut tag + publish
release.yml       tag push → build binaries + DEB + RPM → update-pkgbuild commit
aur-publish.yml   after release.yml → push susshi + susshi-bin to AUR
```

**release-plz uses a PAT (`RELEASE_PLZ_TOKEN`)**, not the built-in `GITHUB_TOKEN` — this is required so the tag it pushes triggers `release.yml`. It acts as the repo owner, not as a bot account. See [.github/SECRETS.md](.github/SECRETS.md) for rotation procedures for this and other repo secrets (`APT_GPG_PRIVATE_KEY`, etc).

**Release PR branches** are named `release-plz-<timestamp>`. The CI `preflight` job detects this pattern and skips the full suite (version-bump PRs contain no code change).

**`update-pkgbuild`** (last job in `release.yml`) commits refreshed `PKGBUILD`, `PKGBUILD.bin`, `.SRCINFO` to master using `RELEASE_PLZ_TOKEN`. The script is at `scripts/update-pkgbuild.sh` and downloads `susshi-linux-x86_64` to compute the binary checksum.

## Test coverage

Coverage target: **≥ 80 %** on testable files (UI, `main.rs`, `i18n.rs` excluded).

Files excluded from measurement:
- `src/ui/**` — requires a live terminal, untestable
- `src/main.rs` — binary entry point
- `src/i18n.rs` — static string table

Tests that use `App::new()` must clear persisted state after construction — `state::load_state()` populates `expanded_items` and `favorites` from `~/.local/share/susshi/state.json` on the dev machine, which pollutes assertions.
