# ── Error dialog ─────────────────────────────────────────────────────────────
error-title = ⚠  Error
error-dismiss = Press Enter or Esc to close

# ── Connection tabs ───────────────────────────────────────────────────────────
tab-title = Connection Mode (Tab to switch)
tab-direct = Direct [1]
tab-jump = Jump [2]
tab-wallix = Wallix [3]

# ── Verbose toggle ────────────────────────────────────────────────────────────
verbose-title = Options (v to toggle)
verbose-label = Verbose (-v)

# ── Search bar ────────────────────────────────────────────────────────────────
search-idle-hint = Press / to search...
search-title-idle = Search (press /)
search-placeholder = (search by name or host, ESC to cancel)
search-title-active = 🔍 Search by name/host ({ $total } servers)
search-no-results = 🔍 No results for '{ $query }'
search-all-match = 🔍 All { $count } servers match
search-partial = 🔍 { $found } / { $total } servers
search-result-all = ✓ Showing all { $count } servers
search-result-partial = ✓ { $found } / { $total } servers match '{ $query }'

# ── Main panels ───────────────────────────────────────────────────────────────
panel-servers = Servers
panel-details = Details
details-placeholder = Select a server to view details.
details-namespace = 📦 Namespace: { $label }
details-group = Group: { $name }
details-environment = Environment: { $group } / { $env }

# ── Details panel labels ──────────────────────────────────────────────────────
label-name = Name:
label-host = Host:
label-port = Port:
label-user = User:
label-mode = Mode:
label-key = Key:
label-jump = Jump:
label-wallix = Wallix:
label-options = Options:

# ── Probe / diagnostics ───────────────────────────────────────────────────────
probe-section = ─── System ──────────────────────
probe-hint =   d — probe
probe-running = Running probe…
probe-kernel = Kernel
probe-cpu = CPU
probe-cpu-cores = Cores
probe-os = OS
probe-load = Load
probe-ram = RAM
probe-disk = Disk /
probe-wallix-error = Probe unavailable in Wallix mode
probe-disk-extra = Disk { $mount }
probe-fs-absent = ⚠  { $mount } — not mounted

# ── Status bar ────────────────────────────────────────────────────────────────
status-normal = Navigate: ↑/↓ | Expand: Space/Enter | Search: / | Mode: Tab/1-3 | v: Verbose | y: Copy | d: Probe | f: Fav | F: Favs | r: Reload | x: Cmd | H: Sort | C: Collapse all | q: Quit
status-searching = Search Mode: Type to filter | ESC: Cancel | Ctrl+U: Clear | Enter: Apply
status-search-active = Navigate: ↑/↓ | Clear: ESC | New search: / | Verbose: v | Enter: Connect | q: Quit

# ── Keyboard hints ────────────────────────────────────────────────────────────
hint-navigate = navigate
hint-validate-cancel = apply / cancel
hint-clear = clear
hint-connect = connect
hint-clear-filter = clear filter
hint-new-search = new search
hint-quit = quit
hint-expand = expand
hint-search = search
hint-mode = mode
hint-tunnels = tunnels
hint-probe = probe
hint-command = command
hint-scp = SCP
hint-copy-ssh = copy SSH
hint-favorite = favorite
hint-favorites-view = ★ favorites view
hint-reload = reload
hint-recent-sort = recent sort
hint-collapse = collapse
hint-expand-all = expand all
hint-verbose = verbose
hint-theme-toggle = next theme
hint-mouse-toggle = text selection
mouse-capture-off = [mouse disabled — text selection active]

# ── Wallix selector overlay ───────────────────────────────────────────────────
wallix-selector-title = Wallix Selection
wallix-selector-loading = Loading Wallix entries for { $server }…
wallix-selector-loading-hint = Contacting the bastion and reading the interactive menu.
wallix-selector-cancel-hint = Esc/q: cancel
wallix-selector-error = Wallix selector error for { $server }
wallix-selector-close-hint = Enter/Esc/q: close
wallix-selector-choose = Select the Wallix entry for { $server } ({ $host })
wallix-selector-list-hint = ↑/↓: navigate | Enter: connect | Esc/q: cancel

# ── Include warnings ──────────────────────────────────────────────────────────
include-warn-load = Failed to load '{ $label }' ({ $path }) : { $error }
include-warn-circular = Circular dependency ignored: '{ $label }' ({ $path })
include-warn-nested = Nested includes in '{ $label }' are ignored (v0.7)

# ── Status messages ───────────────────────────────────────────────────────────
copied = Copied: { $cmd }
clipboard-error = Clipboard error: { $error }
clipboard-unavailable = Clipboard unavailable
ssh-error = SSH error: { $error }

# ── Connection history ────────────────────────────────────────────────────────
last-seen-label = Last conn.:
last-seen-never = —
last-seen-ago = { $duration } ago
last-seen-just-now = just now

# ── Hot reload ────────────────────────────────────────────────────────────────
config-reloaded = Config reloaded ({ $count } servers)
config-reload-error = Config reload error

# ── Favorites ────────────────────────────────────────────────────────────────
favorites-title = ⭐ Favorites
favorite-added = ⭐ Added to favorites
favorite-removed = Removed from favorites

# ── Sort by recency ───────────────────────────────────────────────────────────
sort-recent-on = Sort: recent [H]
sort-recent-off = Sort: alpha  [H]

# ── Ad-hoc command ────────────────────────────────────────────────────────────
cmd-prompt = Command:
cmd-running = Running…
cmd-exit-err = Error (exit { $code })

# ── YAML validation ───────────────────────────────────────────────────────────
validation-title = ⚠  Configuration warnings
validation-unknown-field = { $file } ({ $context }): unknown field "{ $field }"

# ── SSH tunnels ───────────────────────────────────────────────────────────────
tunnel-wallix-unavailable = SSH tunnels unavailable in Wallix mode
tunnel-not-found = Tunnel #{ $index } not found for this server
tunnel-already-active = Tunnel '{ $label }' already active (port { $port })
tunnel-started = Tunnel '{ $label }' started on port { $port }
tunnel-error = Tunnel error: { $error }
tunnel-stopped = Tunnel '{ $label }' (port { $port }) stopped
tunnel-died = Tunnel '{ $label }' (port { $port }) died: { $reason }
tunnel-deleted = Tunnel deleted
tunnel-updated = Tunnel updated
tunnel-added = Tunnel added
tunnel-overlay-new = + (new tunnel)
tunnel-overlay-hints1 =   ↑↓ navigate   Enter start/stop   Del delete
tunnel-overlay-hints2 =   e edit        a add              q/Esc close
tunnel-form-edit-title = Edit tunnel — { $server }
tunnel-form-new-title = New tunnel — { $server }
tunnel-form-field-label =   Label       :
tunnel-form-field-local-port =   Local port  :
tunnel-form-field-remote-host =   Remote host :
tunnel-form-field-remote-port =   Remote port :
tunnel-form-hint =   Tab next field   Enter validate   Esc cancel
tunnel-form-local-port-invalid = Invalid local port (1–65535 expected)
tunnel-form-remote-host-empty = Remote host required
tunnel-form-remote-port-invalid = Invalid remote port (1–65535 expected)
tunnel-badge-label = Tunnels:
tunnel-badge-active =
    { $n_run ->
        [one]  { $n_run } active / { $n_cfg ->
            [one]  { $n_cfg } configured
           *[other] { $n_cfg } configured
        }
       *[other] { $n_run } active / { $n_cfg ->
            [one]  { $n_cfg } configured
           *[other] { $n_cfg } configured
        }
    }
tunnel-badge-none =
    { $n_cfg ->
        [one]  { $n_cfg } configured, none active
       *[other] { $n_cfg } configured, none active
    }

# ── SCP ───────────────────────────────────────────────────────────────────────
scp-wallix-unavailable = SCP unavailable in Wallix mode
scp-done-ok = SCP complete ✔
scp-done-err = SCP completed with errors ✗
scp-failed = SCP failed: { $error }
scp-form-local-required = Local path required
scp-form-remote-required = Remote path required
scp-direction-title = SCP Transfer — { $server }
scp-direction-upload-label = Upload
scp-direction-download-label = Download
scp-direction-upload = (local → server)
scp-direction-download = (server → local)
scp-direction-hint =   Esc cancel
scp-form-title = SCP { $direction } — { $server }
scp-form-field-local =   Local  :
scp-form-field-remote =   Remote :
scp-form-hint =   Tab switch field   Enter confirm   Esc cancel
scp-result-title = SCP Result
scp-result-success = SCP { $direction } completed successfully
scp-result-errors = SCP { $direction } completed with errors
scp-result-fail = SCP error: { $error }
scp-result-hint =   Enter / Esc  close
scp-in-progress = SCP { $direction } in progress...
scp-eta-label = ETA

# ── Credential input dialog ───────────────────────────────────────────────────
credential-input-title-passphrase = SSH Key Passphrase — { $server }
credential-input-title-password = SSH Password — { $server }
credential-input-prompt-passphrase =   Passphrase :
credential-input-prompt-password =   Password   :
credential-input-hint =   Enter confirm   Esc cancel
probe-cm-label = ControlMaster  
probe-cm-active = active
probe-cm-inactive = inactive

# ── Help keyboard overlay ─────────────────────────────────────────────────────
help-title = Keyboard shortcuts  (h / Esc to close)
help-navigate-down = Move down
help-navigate-up = Move up
help-connect = Connect
help-change-mode = Change mode (Direct / Jump / Wallix)
help-select-mode = Select mode directly
help-search = Search
help-close = Exit search / close
help-quit = Quit susshi
help-expand-group = Expand / collapse a group
help-favorite = Add / remove from favorites
help-favorites-view = Show favorites only
help-recent-sort = Sort by last connection
help-collapse-all = Collapse all groups
help-expand-all = Expand all groups
help-theme-toggle = Cycle Catppuccin theme (Latte→Frappe→Macchiato→Mocha)
help-mouse-toggle = Disable mouse capture (native text selection)
help-reload = Reload configuration
help-verbose = SSH verbose mode
help-copy-ssh = Copy SSH command
help-credential = Enter credential (passphrase / password)
help-command = Run ad-hoc command (↑/↓ for history)
help-tunnels = Manage SSH tunnels
help-scp = SCP transfer
help-probe = SSH diagnostic (probe)
help-overview = Overview dashboard for selected group
help-pin = Pin / unpin server (split pane)
help-help = Show / hide this help
help-goto-top-bottom = Go to top / bottom of list
help-page = Previous / next page

# ── First-run wizard ──────────────────────────────────────────────────────────
wizard-title = 👋 Welcome — let's add your first server
wizard-field-group = Group name:
wizard-field-server = Server name:
wizard-field-host = Host:
wizard-field-user = User (optional):
wizard-hint = Tab: next field · Enter: create · Esc: skip (start with an empty config)
wizard-error-group-name = Group name is required
wizard-error-server-name = Server name is required
wizard-error-host = Host is required
wizard-created = ~/.susshi.yml created — welcome aboard!
