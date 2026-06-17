# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.13.0] - 2026-06-17

### Added
- `update_project(current_name, new_name?, new_path?)`: rename a project and/or change its filesystem path while preserving its reserved range and every registered port. At least one of `new_name`/`new_path` must be provided; an empty `new_path` clears the stored path; renaming onto a name already in use is rejected. Wired end-to-end - DB CRUD (`Database::update_project`), `actions::update_project`, the `update_project` socket method, `portsage_client::Client`, the Local/Remote `BackendClient` adapter, the `update_project` Tauri command, the MCP tool (added to the install allowlist), and a `portsage rename <current> [new_name] [--path]` CLI subcommand
- App UI: a pencil button in the `ProjectDetail` header opens an inline `EditProjectForm` to rename a project or change its path; the current selection survives the rename because it is keyed by the stable project id

## [0.12.1] - 2026-05-14

### Fixed
- `import_data` is now atomic: the imported SQLite blob is written to a sibling tmp path, validated via `PRAGMA integrity_check`, then renamed over the live database. A corrupt or truncated `.portsage` archive now fails cleanly instead of clobbering the running database
- After `import_data` the running app reopens its `Arc<Database>` connection (new `Database::reopen`) so the UI observes the imported state without an app restart - the previous in-memory connection still pointed at the replaced inode and was returning stale rows
- `export_data` now propagates the `get_config` errors instead of substituting hardcoded `"4000"` / `"10"` defaults. A silent default in a backup file is worse than a failed export
- MCP install / uninstall from the Tauri Settings panel now go through the same atomic-tmp-then-rename helper used by the CLI - a process kill mid-write can no longer leave the user with a half-truncated `~/.claude.json`

### Changed
- Project / port commands (`delete_project`, `add_port`, `remove_port`, `kill_project`) now take the project name directly. The previous id-based signatures triggered a full `list_all` round-trip on every write to resolve the id; the frontend already had the name. This is a Tauri command surface change only - the socket wire protocol is unchanged
- Tauri MCP install / uninstall delegate to the new `portsage-mcp` crate, eliminating the duplicate implementation that lived in `commands.rs`. The CLI (`portsage mcp install`) uses the same crate, so the two paths cannot drift again

### Internal
- New `crates/portsage-mcp` workspace member owns the Claude config / skill / permissions logic shared between the GUI and CLI installers
- `paths::tests::socket_path_matches_portsage_client_default` pins parity between `paths::socket_path` (backend) and `portsage_client::default_socket_path` (CLI / MCP client) so any future drift is caught at `cargo test`
- `commands.rs` no longer calls `dirs::*` directly - all path resolution goes through `paths.rs` per the CLAUDE.md rule
- `--font-pixel` design token (dead) removed from `index.css`; `--accent-success` (in use but undocumented) added to `docs/DESIGN.md`
- Documentation restructured: `ARCHITECTURE`, `DESIGN`, `ROADMAP`, `RELEASING`, `feature-proposals` moved under `docs/`; new `docs/DATABASE_SCHEMA.md`

## [0.12.0] - 2026-05-12

### Added
- `portsage mcp install / uninstall / status`: canonical CLI path to manage the Claude Code MCP integration, works without the GUI running. The four MCP source files (`server.py`, `pyproject.toml`, `uv.lock`, `SKILL.md`) are embedded into the CLI binary via `include_str!` so a Linux tarball install has no missing files. The install extracts them to `<data_dir>/portsage/mcp/` (Linux: `~/.local/share/portsage/mcp/`, macOS: `~/Library/Application Support/portsage/mcp/`), runs `uv sync`, registers in `~/.claude.json` (or `./.mcp.json` with `--project`), copies the SKILL.md to `~/.claude/skills/portsage/`, and adds the 14 tool entries to `~/.claude/settings.json`. All JSON edits go through a parse-or-bail + atomic-tmp-then-rename helper so a corrupt config is never silently overwritten
- `portsage self-update`: compares `env!("CARGO_PKG_VERSION")` against the latest GitHub release tag (fetched via `curl` to avoid pulling a TLS dep into the CLI). On macOS with brew available, runs `brew update && brew upgrade --cask portsage` after a confirmation (`-y` to skip). On Linux it prints the release URL rather than overwriting a binary held open by systemd

### Fixed
- MCP socket path resolution on Linux: `mcp/server.py` now mirrors `portsage_client::default_socket_path` (env override > macOS Application Support > Linux `$XDG_RUNTIME_DIR` with `/tmp/portsage-<uid>.sock` fallback). The previous `~/.config/portsage/portsage.sock` Linux default never matched the Rust side and would have broken every MCP call on a Linux dev box

## [0.11.0] - 2026-05-12

### Added (multi-host evolution, see `docs/multi-host-evolution.md`)
- Linux x86_64 headless server (`portsage-server`): same binary as the macOS app built with `--no-default-features`, drops the Tauri runtime entirely. Includes systemd unit + idempotent `install.sh` in `packaging/linux/`, CI tarball workflow on tag push (`.github/workflows/server-build.yml`), per-OS scanner (macOS lsof, Linux `/proc/net/tcp` + `ss` fallback), XDG paths on Linux (`$XDG_DATA_HOME` for the DB, `$XDG_RUNTIME_DIR` for the socket), `--socket <path>` flag and `PORTSAGE_SOCKET` env override
- Remote backend in the Mac UI: configure remote Portsage servers via Settings -> Remote backends, switch between Local and Remote in the sidebar dropdown, every existing project/port command routes through the active backend. Backed by `remote_backends` SQLite table, `BackendManager` owning per-backend SSH unix-socket tunnels with state machine and per-backend mutex, `BackendRouter` with persisted current target, 10 new Tauri commands, `tunnel://state-changed` events, `useBackends` hook
- Auto SSH port forwarding: when a remote backend has registered ports, Portsage opens `ssh -O forward -L <port>:localhost:<port>` against the same ControlMaster the protocol tunnel uses. Three-state port row arrow indicator (active / failed / cancelled) with click-toggle, local-port collision probe with process name + pid in the failure reason, ControlMaster ownership (piggyback on user's `ssh_config` `ControlMaster auto` when available, otherwise open + track our own at `paths::state_dir()/cm-<alias>.sock`), startup auto-sync + 60s periodic safety-net timer (`forwards::start_auto_sync`), per-backend "Excluded ports" sub-UI in Settings, clean shutdown of Portsage-managed ControlMasters on app quit
- CLI `--backend <name>` flag (also `PORTSAGE_BACKEND` env): the CLI asks the Mac app's socket for the named backend's local-side forwarded socket and connects to it directly. Requires the Tauri app to be running with the tunnel open

### Fixed (during the multi-host live smoke test on the `forge` dev box)
- Socket file 0600 inside systemd's 0750 `RuntimeDirectory` blocked group access. The scanner now picks 0660 when the parent directory is externally managed (systemd-style shared group install) and keeps 0600 for the per-user XDG install
- `install.sh` rewrites both `User=` and `Group=` in the systemd unit to the target dev user. The kernel's `__ptrace_may_access(PTRACE_MODE_FSCREDS)` requires `fsuid` AND `fsgid` to match the target process's creds for `/proc/<other_pid>/fd/*` readlinks - with the original `Group=portsage` (gid 987) every port owned by the dev user (gid 1000) showed `?` in the Process column

## [0.10.0] - 2026-05-11

### Added
- `WelcomePanel`: replaces the bare "Select a project from the sidebar" empty state. First-run shows a tagline, a hero CTA "Create your first project", and a "Next steps" checklist with a deep-link to Settings -> Integrations. With projects already present, shows Projects/Registered ports/Active stat cards plus quick buttons to create, review unmanaged ports, or open settings
- `UITabs` / `UITabPanel` primitive in `src/components/ui/`: mono-styled tabs with amber active underline, optional count badge, full `role="tablist"` / `role="tab"` / `role="tabpanel"` aria wiring
- `Settings` panel split into three tabs - **General** (autostart + port range), **Integrations** (Claude Code MCP + Other editors), **Data** (export/import). "Other editors" config moved into an accordion (closed by default) to stop the panel from scrolling so much at first open. The current tab is deep-linkable via a controlled `tab` prop, so other panels can jump straight to the right section
- `warning` variant on `UIButton` (amber): for actions that are destructive against the *target* (kill a process) but not against portsage data. Sits between `ghost` and `danger`

### Changed
- Sidebar: projects with active ports now display an amber status dot to the left of the name and the name in `text-primary`; inactive projects keep `text-secondary` with no dot. The amber badge with the active count stays. Scanning "what is running right now" is now immediate. Empty search shows "No matches" instead of a silent empty list
- `PortRow`: when a port is inactive the service name and port number are dimmed to `text-muted`, so registered-but-not-running rows visually recede
- `PortRow`, `ProjectDetail`, `UnmanagedPortsPanel`: kill-process button switched from neutral `ghost` to the new `warning` (amber) variant. The Power icon is kept; the color change makes it clear it's a real action and not just decoration
- `ProjectDetail` toolbar split into two groups separated by a vertical divider: navigation actions (Open in Finder / Open in Terminal) on the left, destructive actions (kill all / delete project) on the right
- `PopoverPanel` footer cleaned up: Quit moved to a low-key icon button in the header (it was sitting at the same visual weight as the primary "Open portsage" CTA). The footer now shows only the active/total count and the open button. Inactive services in the popover list also dimmed to `text-secondary`
- `UIPortLink`: focus-visible now adds an amber ring (previously only a color change), and `aria-label` is set explicitly so screen readers announce "Open http://localhost:PORT in browser"
- All icon-only buttons across the app gained explicit `aria-label` (folder, terminal, kill, delete, quit, port-link), and their inner Lucide icons are marked `aria-hidden="true"`

## [0.9.1] - 2026-05-11

### Fixed
- CLI autospawn timeout raised from 3s to 8s. The first `portsage <cmd>` after a fresh install was occasionally timing out before the freshly-spawned headless backend bound the socket - cold launches on macOS pay a Gatekeeper xattr scan + dyld cache priming cost that can take several seconds. Warm spawns are unaffected

### Changed
- `-y` / `--yes` promoted to a global flag. Both `portsage release foo -y` and `portsage -y release foo` work; previously only the first did, and the second confusingly errored with `unexpected argument '-y'`

## [0.9.0] - 2026-05-11

### Added
- `portsage` CLI bundled inside the .app and exposed on PATH via the Homebrew cask. Full parity with the MCP server: `list`, `status`, `reserve`, `register`, `remove`, `release`, `scan`, `kill`, `kill-project`, `open`, `config get|set`, `doctor`. `--here` derives the project from the current directory. Output modes: human (colored on TTY), `--json` for scripting, `--quiet` for pipes. Destructive ops require interactive confirmation or `-y`. Exit codes: `0` ok, `1` generic, `2` usage, `3` backend unreachable, `4` not found, `5` conflict
- `--headless` (`-H`) mode: the Tauri binary runs the Unix-socket server only, with no tray or windows. Used by the CLI to auto-spawn the backend if no instance is running. SIGINT/SIGTERM trigger clean shutdown. Probes the socket first and exits cleanly when another instance (GUI or headless) is already serving
- Extended the Unix socket protocol with 9 new methods to match the GUI surface: `remove_port`, `list_unmanaged`, `next_range`, `get_config`, `set_config` (whitelists `base_port` / `range_size`), `kill_port`, `kill_project`, `open_in_browser`, `find_project_by_path`. `list_all` now returns project ids, ranges, `created_at`, and per-port `pid` / `process`; `reserve_range` returns the new project's id; `register_port` returns the new port's id
- MCP server gained the matching 9 tools so Claude can drive the full surface (kill zombies, open URLs, peek the next range, mutate config, look up projects by path)

### Changed
- Repo restructured into a Cargo workspace: `src-tauri/` (the existing Tauri app, unchanged in place), `crates/portsage-client/` (sync `UnixStream` client + wire types - single source of truth, consumed by both the app's tests and the CLI), `crates/portsage-cli/` (clap-based binary). One shared `Cargo.lock` at the root, one shared `target/`
- `src-tauri/src/commands.rs` is now a thin Tauri wrapper over the new `actions.rs` module, which hosts pure logic shared between the Tauri commands and the socket dispatcher
- All workspace crates inherit `version` from `[workspace.package]` in the root `Cargo.toml`, so `portsage --version` always matches the app release

## [0.8.3] - 2026-05-11

### Changed
- Kill (Power) buttons use the neutral ghost variant instead of danger red. Stopping a process is reversible and doesn't warrant the same alarm color as project/port deletion; the trash icon remains red for true destructive actions

## [0.8.2] - 2026-05-11

### Fixed
- Button hover is now clearly visible: ghost hover uses `white/10`, danger hover uses `accent-danger/20` - both work on any background (bg-deep, bg-surface, bg-elevated)
- Port rows now have a visible hover state (`bg-elevated`) so it's obvious which row is targeted and that hover-reveal actions appeared
- Kill button no longer renders on inactive ports (previously it was visible at `disabled` opacity even before hover) - the action slot stays present so row alignment is preserved

## [0.8.1] - 2026-05-11

### Changed
- Icon-only buttons are square (28x28 in toolbars, 24x24 in port rows): added `size="icon"` and `size="icon-sm"` to `UIButton`
- Port row layout reworked: PID is now a column of its own (no more inline middle-dot separator)
- `ProjectDetail` shows column headers above the port list: Service, Process, PID, Port
- Per-row action slots (kill, remove) reserve their width even when not applicable, so rows stay vertically aligned regardless of port state

## [0.8.0] - 2026-05-11

### Added
- Click on any port number to open `http://localhost:PORT` in the default browser, available in the main window project detail, the menubar popover, and the unmanaged ports panel. Backed by a new `open_in_browser` Tauri command and a `UIPortLink` primitive
- Stop process by port: Power icon in PortRow (per port) and in the project detail header (per project), and on each unmanaged port row. Sends SIGTERM, waits 2s, escalates to SIGKILL if the process is still alive. Confirmation dialog lists processes and PIDs; toasts report the outcome (`terminated`, `killed`, `not_active`, `permission_denied`). Backed by new `kill_port` and `kill_project` async Tauri commands. PID is now part of `PortStatus` so the UI can display it alongside the process name

## [0.7.2] - 2026-04-07

### Fixed
- Duplicate tray icons when more than one Portsage process was running (e.g. dev build + installed .app, or accidental relaunch): added `tauri-plugin-single-instance` so a second launch focuses the existing instance instead of spawning a new one
- Main window now opens at first launch again (regression introduced in 0.7.1 where the window was hidden at startup)

## [0.7.1] - 2026-04-07

### Fixed
- Tray icon missing at launch: the main window no longer opens visible at startup, so the app starts as a pure menubar app and the status bar icon is shown immediately
- Reopening the app from Spotlight/Finder/Dock after closing it with the X now correctly re-shows the main window via `RunEvent::Reopen`, instead of leaving a dock icon with no UI and forcing a force-quit

## [0.6.1] - 2026-04-07

### Added
- Error toast in the main window: failed actions (duplicate name, port outside range, etc.) now surface as a dismissible bottom-right banner instead of being silently logged to the console
- `humanizeError` translation layer: common SQLite/IO errors are mapped to readable messages, with raw text as fallback
- Documentation of the MCP Unix socket protocol in `PROJECT.md` (transport, methods, request/response envelopes, example session)

### Changed
- Hardened the MCP Unix socket: parent dir is now `0700` and the socket file `0600`, so only the current user can connect
- Idle socket connections are closed after 60 seconds, preventing leaked client tasks from accumulating
- Python MCP client (`mcp/server.py`) now applies a 5s timeout on connect/send/recv, so a stuck backend can no longer hang Claude Code
- Centralized the database path: `commands::export_data` and `commands::import_data` now use `Database::db_path()` instead of recomputing it
- Toast position: bottom-right (was bottom-center)

### Fixed
- Race condition in `Database::create_project`: range computation and insert are now performed under a single mutex lock, so two concurrent project creations can no longer produce overlapping port ranges
- Replaced all `.expect()` calls in critical startup paths (`lib.rs`, `socket.rs`) with graceful error handling - the app no longer panics on DB init or socket bind failures
- Replaced all `Mutex::lock().unwrap()` calls in `db.rs` with a poisoning-safe helper, eliminating the theoretical panic on a poisoned mutex
- Removed `Path::parent().unwrap()` in `commands::install_mcp`

### Planned
- Copy URL for HTTP ports
- CLI for scripting
- i18n and language switcher (English + Italian)
- Project tags and colors
- System notifications

## [0.6.0] - 2026-04-07

### Added
- Initial public release as **Portsage** (renamed from internal codename `grimport`)
- macOS menubar app with popover and full window
- SQLite-backed project registry with port range allocation
- Real-time port scanning via `lsof` with process name resolution
- Unmanaged ports detection for active ports above 3000
- MCP server (Python, thin client) connecting to a local Unix socket
- One-click MCP installation for Claude Code from Settings
- Copy-paste MCP config for Cursor, Claude Desktop, Cline, VS Code Copilot, Codex (TOML), Windsurf
- Auto-launch at login (toggle in Settings)
- Configurable `base_port` and `range_size`
- Export and import data as `.portsage` archive (SQLite dump + config)
- "Open in Finder" and "Open in Terminal" shortcuts on project paths
- Search/filter projects in the sidebar
- Homebrew distribution via `essedev/portsage` tap
- English UI strings throughout the app
- English documentation (README, PROJECT, DESIGN, ROADMAP, FEATURES_TODO, RELEASING) with Italian companions for README and PROJECT
- MIT License

[Unreleased]: https://github.com/essedev/portsage/compare/v0.12.1...HEAD
[0.12.1]: https://github.com/essedev/portsage/compare/v0.12.0...v0.12.1
[0.12.0]: https://github.com/essedev/portsage/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/essedev/portsage/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/essedev/portsage/compare/v0.9.1...v0.10.0
[0.9.1]: https://github.com/essedev/portsage/compare/v0.9.0...v0.9.1
[0.9.0]: https://github.com/essedev/portsage/compare/v0.8.3...v0.9.0
[0.8.3]: https://github.com/essedev/portsage/compare/v0.8.2...v0.8.3
[0.8.2]: https://github.com/essedev/portsage/compare/v0.8.1...v0.8.2
[0.8.1]: https://github.com/essedev/portsage/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/essedev/portsage/compare/v0.7.2...v0.8.0
[0.7.2]: https://github.com/essedev/portsage/compare/v0.7.1...v0.7.2
[0.7.1]: https://github.com/essedev/portsage/compare/v0.6.1...v0.7.1
[0.6.1]: https://github.com/essedev/portsage/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/essedev/portsage/releases/tag/v0.6.0
