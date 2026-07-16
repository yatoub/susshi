# Repository secrets — rotation runbook

Maintainer-only reference. Not part of the public docs site (`docs/`) since
it's operational, not user-facing.

## `RELEASE_PLZ_TOKEN`

A GitHub PAT (classic or fine-grained, `repo` scope) belonging to the repo
owner, used by `release-plz.yml` and `update-pkgbuild` (in `release.yml`).
**Not** the built-in `GITHUB_TOKEN`: GitHub silently skips workflow triggers
for pushes/tags made with the built-in token, so `RELEASE_PLZ_TOKEN` is what
lets a release-plz tag push actually trigger `release.yml`, and what lets
`update-pkgbuild`'s push to master trigger `aur-publish.yml`.

**Blast radius if leaked:** push access to `master`, ability to create
releases and tags, trigger the full publish pipeline (crates.io, GitHub
Releases, APT/RPM/AUR). Treat as equivalent to repo-owner credentials.

**Rotation:**

1. Generate a new PAT at <https://github.com/settings/tokens> (classic) or
   <https://github.com/settings/personal-access-tokens> (fine-grained),
   scoped to this repo only if using fine-grained tokens. Required scope:
   `repo` (classic) or `Contents: read/write` + `Pull requests: read/write`
   (fine-grained).
2. `gh secret set RELEASE_PLZ_TOKEN --repo yatoub/susshi` and paste the new
   token.
3. Revoke the old PAT at the same settings page.
4. No downstream update needed — every workflow reads the secret by name at
   run time.

**Rotate when:** the token owner's account security changes (password/2FA
reset), on any suspicion of leak, or per your own periodic security policy.
GitHub PATs don't auto-expire unless you set an expiration at creation —
consider setting one and rotating proactively before it lapses.

## `APT_GPG_PRIVATE_KEY`

Signs **both** the APT repo (`publish-apt`) and the RPM repo (`publish-rpm`)
in `release.yml` — one key, one trust root for both package managers.

**Blast radius if leaked:** an attacker could sign malicious `.deb`/`.rpm`
packages that `apt`/`dnf` clients would accept as authentic from
`yatoub.github.io/susshi/{apt,rpm}`. Treat as high severity.

**Rotation / revocation:**

1. Generate a new key pair (same recipe used for the original key):
   ```bash
   gpg --batch --full-generate-key <<EOF
   Key-Type: RSA
   Key-Length: 4096
   Name-Real: Susshi APT Repo
   Name-Email: releases@yatoub.dev
   Expire-Date: 2y
   %no-protection
   %commit
   EOF
   ```
2. Export and install the new private key:
   ```bash
   gpg --list-secret-keys --keyid-format=long   # get the new KEYID
   gpg --armor --export-secret-keys <KEYID> > private.asc
   gh secret set APT_GPG_PRIVATE_KEY --repo yatoub/susshi < private.asc
   shred -u private.asc
   ```
3. Export the new public key and replace `docs/apt-pubkey.asc` in a PR —
   this is the key end users fetch and trust
   (`curl .../apt-pubkey.asc | gpg --dearmor ...`), so existing installs
   keep working only after they re-import it:
   ```bash
   gpg --armor --export <KEYID> > docs/apt-pubkey.asc
   ```
4. Re-trigger `release.yml` via `workflow_dispatch` for the current tag so
   the APT/RPM repos get re-signed with the new key
   (`publish-apt`/`publish-rpm` already run on `workflow_dispatch` — see
   #156). `smoke-test-apt`/`smoke-test-rpm` (see #160) will catch a bad
   signature automatically.
5. If the old key was compromised (not just routine rotation), also
   generate a revocation certificate for it and publish that as a heads-up
   in a GitHub Release note or issue — GPG has no way to "unpublish" trust
   in a key once users have imported it, so this is discoverability, not
   enforcement.

**Rotate when:** on suspicion of leak (treat as urgent — see step 5), or
proactively before the key's `Expire-Date` lapses (2 years from creation;
check with `gpg --list-secret-keys --keyid-format=long`, `expires` field).

## Other secrets (lighter-touch, rotate via the same "gh secret set" pattern)

| Secret | Used by | Notes |
|---|---|---|
| `AUR_SSH_PRIVATE_KEY`, `AUR_USERNAME`, `AUR_EMAIL` | `aur-publish.yml` | SSH deploy key for the AUR git repos. Rotate via AUR's own account SSH key management, then update the secret. |
| `CARGO_REGISTRY_TOKEN` | `release-plz.yml` | crates.io publish token. Rotate at <https://crates.io/settings/tokens>. |
| `CODECOV_TOKEN` | `ci.yml` (coverage upload) | Low sensitivity — regenerate at codecov.io if leaked, no publish/write capability tied to it. |

## General notes

- All secrets are set via `gh secret set <NAME> --repo yatoub/susshi`.
- After rotating any secret used by `release.yml`, a `workflow_dispatch`
  re-run against the current tag is the fastest way to confirm the new
  credential actually works end-to-end before the next real release depends
  on it.
