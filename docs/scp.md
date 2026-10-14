# SCP File Transfers

susshi provides an in-TUI form for uploading and downloading files via SCP.

## Opening the SCP Form

Press `s` when a server is selected to open the SCP transfer form.

## Form Fields

| Field | Description |
| --- | --- |
| Source | Local path (upload) or `user@host:/remote/path` (download) |
| Destination | Remote path (upload) or local path (download) |

The form pre-fills the remote side with `user@host:` based on the selected server.

## Transfer Progress

Transfer output is captured through a PTY-backed OpenSSH invocation and displayed live in the overlay. The progress line updates as data is transferred.

Dismiss with `Esc` once the transfer is complete, or press `Esc` to cancel an in-progress transfer.

## Notes

- The transfer uses the same SSH parameters as the selected server (key, port, jump, agent socket).
- Wallix mode is supported through the system `scp` binary: susshi runs `scp -A … user@target+SSH+authorization+bastion_user@bastion:path`. Authorizations come from the configuration (`wallix.authorization`, then `wallix.authorizations`, tried in order while Wallix answers `Cible invalide`); the name derived from the YAML structure (`<ENV>-<GROUP>_<group>`) is only a last resort. See [Wallix Guide](wallix.md#declaring-authorizations-recommended). Agent forwarding (`-A`) lets the bastion authenticate on the target with your SSH agent, so your key must be loaded in the agent. Progress is only reported when the transfer ends.
- For large or automated transfers, use `rsync` or `scp` directly with the SSH command copied via `y`.
