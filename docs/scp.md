# SCP File Transfers

susshi provides an in-TUI form for uploading and downloading files to the selected server.

## Opening the SCP Form

Press `s` when a server is selected, then choose the direction:

| Key | Direction |
| --- | --- |
| `↑` / `u` | Upload (local → server) |
| `↓` / `d` | Download (server → local) |

## Form Fields

| Field | Description |
| --- | --- |
| Local | Local file (upload) or destination path (download) |
| Remote | Remote destination (upload) or remote file (download) — pre-filled with `user@host:~` |

`Tab` switches field, `Enter` starts the transfer, `Esc` cancels.

Remote paths:

- `~` and `~/…` are relative to the remote home directory.
- A path ending with `/` is treated as a directory: the local file name is appended.

## Transfer Progress

The transfer runs in the background: a progress bar with the transferred size and ETA is shown in the status panel while you keep using the TUI. When it ends, a result popup shows the outcome (long error messages wrap); close it with `Enter` or `Esc`.

## How Transfers Run

| Mode | Engine |
| --- | --- |
| Direct | Native SFTP/SCP over libssh2 (agent, `ssh_key`, then default keys) |
| Jump | Native, through the first jump host (`direct-tcpip` channel) |
| Wallix | System `scp` binary, see below |

After an upload, susshi checks the server acknowledgement: a remote write error (permission denied, full disk…) is reported instead of a false success.

### Wallix mode

susshi runs the system `scp` with the direct Wallix login:

```bash
scp -A … user@target+SSH+AUTHORIZATION+bastion_user@bastion:path
```

- Authorizations come from the configuration (`wallix.authorization`, then `wallix.authorizations`), tried in order while Wallix answers `Cible invalide`; the name derived from the YAML structure (`<ENV>-<GROUP>_<group>`) is only a last resort. See [Wallix Guide](wallix.md#declaring-authorizations-recommended).
- Agent forwarding (`-A`) lets the bastion authenticate on the target with your SSH agent: your key must be loaded in the agent (`ssh-add -l`). No password prompt is possible (`BatchMode`).
- Progress is only reported when the transfer ends.
- On failure, the error shows the `scp` message (without OpenSSH warnings) and the exact target used, to compare with a manual `scp`.

## Notes

- The transfer uses the same SSH parameters as the selected server (key, port, jump, agent socket, `ssh_options` in Wallix mode).
- For large or automated transfers, use `rsync` or `scp` directly with the SSH command copied via `y`.
