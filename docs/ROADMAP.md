# Roadmap

## v0.1 - Foundation

- [x] Tauri v2 + React 19 + Tailwind v4 project setup
- [x] SQLite database: schema, migrations, path `~/.config/portsage/`
- [x] Rust backend: project and port CRUD, DB access
- [x] Port scanner: `lsof` wrapper to detect active ports
- [x] Menubar icon + popover with project list and port status
- [x] Full app window: project sidebar + detail with services/ports
- [x] Add/remove projects and services from the UI

## v0.2 - MCP server

- [x] Local Unix socket exposed by the Rust backend (`~/.config/portsage/portsage.sock`)
- [x] Python MCP server (thin client, forwards to socket)
- [x] Tool: `list_all` - full registry plus port status
- [x] Tool: `reserve_range(project_name)` - reserves next free range
- [x] Tool: `register_port(project_name, service, port)` - registers a port
- [x] Tool: `release_project(project_name)` - releases a range
- [x] Tool: `scan_active` - active ports on the machine
- [x] Skill file for Claude Code
- [x] Install script + "Connect to Claude Code" UI in settings

## v0.3 - Polish

- [x] Auto-refresh port status (5s polling)
- [x] Process name visible next to active ports (resolved via `ps`)
- [x] Project search/filter in the sidebar
- [x] Unmanaged ports: detect active ports > 3000 not associated with projects
- [x] Click on project path -> open in Finder or Terminal

## v0.4 - Settings and portability

- [x] Settings: configure base_port and range_size
- [x] Export: DB + preferences in a `.portsage` file (zip with SQLite dump + config)
- [x] Import: restore from a `.portsage` file
- [x] Launch at login (tauri-plugin-autostart)
- [ ] ~~Import ports from docker-compose.yml~~ (covered by MCP)
- [ ] ~~Dark/light mode~~ (the dark theme is the app's identity)

## v0.5 - Distribution

- [x] `.dmg` build with `tauri build`
- [x] Homebrew tap (`brew tap essedev/portsage && brew install portsage`)
- [x] GitHub release with attached DMG
- [ ] Auto-update (Tauri updater) - future

## v0.6 - Feature parity with competitors

See [feature-proposals.md](feature-proposals.md) for the design of the still-open items.

- [x] **Kill process from the UI** - per-port and per-project (SIGTERM with SIGKILL escalation after 2s)
- [x] **Open in browser** - for HTTP ports, click opens `localhost:PORT` in the default browser
- [x] **CLI** - `portsage` command for scripting (`portsage reserve`, `portsage list`, `portsage kill`, etc.), bundled with the app and exposed on PATH via Homebrew
- [ ] **Project tags and colors** - visual customization to recognize projects at a glance
- [ ] **System notifications** - macOS alerts for collisions, zombie ports, MCP events
- [ ] **i18n and language switcher** - proper i18next setup, English + Italian, language switcher in settings, persisted in DB

## v0.7 - CI/CD and cross-platform

- [x] GitHub Action for automatic builds on push/release (`.github/workflows/ci.yml`, `server-build.yml`)
- [ ] Universal macOS binary (arm64 + x86_64)
- [x] Cross-platform tests in CI (macOS lane + Linux lane on every PR)
- [ ] Windows support: Unix socket -> named pipe, scanner via netstat/API, OS-specific commands

## v0.8 - Multi-host (Linux server backend + remote UI + auto-forward)

The full plan lives in [`multi-host-evolution.md`](multi-host-evolution.md). Shipped in 4 phases:

- **Phase 1 - Cross-platform server** (validated live on the `forge` dev box, 2026-05-12): Linux x86_64 headless build (`portsage-server`), scanner abstraction (macOS lsof vs Linux `/proc/net/tcp` + `ss` fallback), XDG paths, systemd unit. Unblocks running portsage on dedicated dev servers and lets remote Claude Code agents talk to a local portsage MCP. Done: scanner abstraction with macOS / Linux impls, XDG path module, `gui` cargo feature gating Tauri so Linux can build headless-only, `--socket` override + `PORTSAGE_SOCKET` env, systemd unit at `packaging/linux/portsage-server.service`, idempotent `packaging/linux/install.sh` (patches `User=`/`Group=` to the target dev user at install time so the kernel's `__ptrace_may_access(PTRACE_MODE_FSCREDS)` match on both `fsuid` and `fsgid` against the dev user's processes - documented in `packaging/linux/README.md`), CI workflows. Remaining: Homebrew-on-Linux integration (low priority, see plan #4.x).
- **Phase 2 - Remote backend in the UI** (validated live on forge incl. Tauri window, 2026-05-12): Mac UI can configure remote Portsage servers, open SSH local-socket tunnels to them, and run every project/port command against the selected backend. Done: `remote_backends` schema + CRUD, `BackendManager` + `SshTunnel` with state machine and per-backend mutex, `BackendRouter` + `BackendClient` adapter, 10 Tauri commands (CRUD + test + tunnel statuses + current target persistence), `BackendSwitcher` in the sidebar with live status dot via `tunnel://state-changed` events, "Remote backends" tab in Settings with add/test/remove/auto-forward toggle, all existing Tauri commands routed through the active backend, `ProjectDetail` hides Finder/Terminal buttons for Remote, CLI `--backend <name>` flag with `PORTSAGE_BACKEND` env (delegates tunnel lifecycle to the Mac app rather than opening its own), `humanizeError` covers SSH-specific failure modes. Smoke-test bugs caught + fixed: socket file 0600 inside systemd's 0750 RuntimeDirectory blocked group access (commit `87b0334`); systemd `User=portsage Group=portsage` blocked `/proc/<pid>/fd` readlink for processes owned by the dev user because the kernel requires both `fsuid` AND `fsgid` to match (commit `d428c47`). Divergence from plan: the CLI does not open tunnels itself, it reads the local-side forwarded socket path from the Mac app's socket and connects to that - the plan section 2.5 said "opens a tunnel just like the UI does", but cross-process tunnel state would mean two `BackendManager` instances racing on the same SSH child.
- **Phase 3 - SSH port forwarding integration** (feature complete, awaits a live smoke test): Mac UI exposes per-port forward toggle, `ForwardManager` owns the (backend, port) -> state map, the `ForwardController` trait shells out to `ssh -O forward / cancel` and gracefully handles "user has ControlMaster" vs "we need to open one ourselves" (managed masters live at `paths::state_dir()/cm-<alias>.sock` and get torn down on app quit). Local-port collision probe surfaces "port X is in use locally by node (pid 12345)" before issuing the ssh call, instead of letting the bind silently fail. Three port-row states render with an arrow icon: active+forwarded (amber), failed (red, hover for reason), cancelled (muted, click to re-open). `forwards::start_auto_sync` runs a daemon thread that on app start opens tunnels + reconciles forwards for every backend with `auto_forward_enabled = true`, then loops every `PERIODIC_SYNC_INTERVAL` (60s) as safety-net for remote-driven mutations (e.g. an MCP agent on the box calling `register_port`). Per-backend "Excluded ports" sub-UI in Settings > Remote backends lets the user blocklist ports that should never be auto-forwarded. Deferred from the original plan: (a) server-pushed `StateChanged` event channel in the socket protocol - we get eventual consistency via the 60s timer, which is acceptable for the dev-server use case; (b) macOS notifications on local port conflict - the failure surfaces in the indicator tooltip + toast.
- **Phase 4 - Quality of life** (not started): project migration between backends, health dashboard in Settings, CLI `portsage backends list / add / remove`, Tailscale host auto-discovery via `tailscale status --json`, multi-tenant remote server support.

Additional Phase 2/3 polish that was in the original plan but deferred as nice-to-have (none blocks the user flow):
- "SSH into project dir" button replacing "Open in Terminal" when the active backend is Remote (currently those buttons are hidden, not replaced).
- Popover backend label (`[dev]`) + click-to-cycle.
- Pre-check `ssh -G <alias>` before opening a tunnel for a faster "Host 'dev' not in your SSH config" message (today the error surfaces verbatim via `humanizeError`).
- "Refresh SSH connection" button for stale ControlMasters.
- Project-level forward summary indicator in the sidebar (per-port indicators exist on the port rows; aggregate icon next to the project name does not).

Effort estimate: 2-3 weeks of focused work, shippable incrementally.

This roadmap entry subsumes the Linux support that was listed in v0.7. Windows support remains in v0.7 as a separate concern.

## v0.9 - CLI + headless mode (shipped: v0.9.0 / v0.9.1)

- [x] `portsage` CLI bundled inside `Portsage.app` and exposed on PATH via the Homebrew cask. Full parity with the MCP surface: `list`, `status`, `reserve`, `register`, `remove`, `release`, `scan`, `kill`, `kill-project`, `open`, `config get|set`, `doctor`. Destructive ops require interactive confirmation or `-y`; output modes: human / `--json` / `-q`.
- [x] `--headless` (`-H`) mode on the Tauri binary: socket-server only, no tray or windows. Used by the CLI to auto-spawn the backend if no instance is running.
- [x] Unix socket protocol extended from 5 to 14 methods (full GUI parity). MCP server picks up the same 9 new tools.
- [x] Cargo workspace split: `src-tauri/` + `crates/portsage-client/` (shared wire types + sync client) + `crates/portsage-cli/`.

## v0.10 - UI overhaul (shipped: v0.10.0)

- [x] `WelcomePanel` for the empty-state main window (first-run CTA + checklist; with-projects stat cards).
- [x] Settings split into three tabs (General / Integrations / Data) with `UITabs` primitive and a `tab` deep-link prop.
- [x] Sidebar active-port amber dot + dimmed inactive entries, hover states on rows, `warning` (amber) button variant for Power buttons.
- [x] `aria-label` on every icon-only button, focus rings on `UIPortLink`.

## v0.11 - Multi-host (shipped: v0.11.0, see v0.8 above for the full plan)

- [x] Phase 1: Linux headless server + systemd unit + idempotent installer + CI tarball workflow.
- [x] Phase 2: Remote backend in the UI (`remote_backends` table, `BackendManager`, `BackendRouter`, sidebar `BackendSwitcher`, live tunnel state events).
- [x] Phase 3: Auto SSH port forwarding (`ForwardManager`, ControlMaster ownership picker, local-port collision probe, per-backend "Excluded ports" sub-UI, 60s safety-net sync).
- [x] CLI `--backend <name>` / `PORTSAGE_BACKEND` env (delegates tunnel lifecycle to the Mac app).
- [ ] Phase 4 (not started): project migration between backends, health dashboard, CLI `portsage backends list / add / remove`, Tailscale host auto-discovery.

## v0.14 - Recover and prune (shipped: v0.14.0)

- [x] **Trash**: `release` and `remove` archive a snapshot for 30 days instead of dropping it. Restoring a project brings back its original range, so its `.env` and compose files keep working. Sidebar row that appears only when the trash is non-empty, `portsage trash list|restore|purge`, `list_trash` / `restore_trash` MCP tools, and an Undo button in the toast right after a deletion.
- [x] **Prune**: projects whose folder is gone or untouched for N days (default 90) can be archived in one pass. Archiving keeps name, range and ports; a project un-archives itself when one of its ports starts listening. Signal is `max(dir mtime, .git/logs/HEAD mtime)`, validated against `git log -1` on 25 real repos. Sidebar "Prune" row + "Archived" section, `portsage prune|archive|unarchive`, read-only `list_stale` MCP tool.
- [x] **Docker kill fixed**: the docker CLI is resolved explicitly, because an app bundle launched by launchd has no docker on its PATH. `KillOutcome` now distinguishes CLI missing / daemon down / no container / stop failed.
- Deliberately not done: recycling the ranges of released projects. Numbers are not scarce (27 projects reach 4340 of 65535) and reusing a range would point an old `.env` at another project's service.

## v0.12 - CLI-driven MCP install + self-update (shipped: v0.12.0)

- [x] **`portsage mcp install / uninstall / status`** - canonical install path, works without the GUI running. The four MCP source files (`server.py`, `pyproject.toml`, `uv.lock`, `SKILL.md`) are embedded into the CLI binary via `include_str!` so a Linux tarball install has no missing files. The install extracts them to `<data_dir>/portsage/mcp/` (Linux: `~/.local/share/portsage/mcp/`, macOS: `~/Library/Application Support/portsage/mcp/`), runs `uv sync`, registers in `~/.claude.json` (or `./.mcp.json` with `--project`), copies the SKILL.md, and adds the 14 tool entries to `~/.claude/settings.json`. All JSON edits go through a parse-or-bail + atomic-tmp-then-rename helper so a corrupt `~/.claude.json` is never silently overwritten. Tests cover round-tripping a synthetic config and ensuring sibling entries are preserved.
- [x] **`portsage self-update`**: compares `env!("CARGO_PKG_VERSION")` against the latest GitHub release tag (fetched via `curl` to avoid pulling a TLS dep into the CLI). On macOS with brew available, runs `brew update && brew upgrade --cask portsage` after a `--yes`-able confirmation. On Linux it just prints the release URL - the binary lives in `/usr/local/bin/` and is held open by a systemd unit, so in-place replacement under sudo isn't worth the risk for one fewer command.
- [x] **MCP socket path resolution fixed for Linux**: `mcp/server.py` now mirrors `portsage_client::default_socket_path` (env override > macOS Application Support > Linux `$XDG_RUNTIME_DIR` with `/tmp/portsage-<uid>.sock` fallback). The previous `~/.config/portsage/portsage.sock` Linux default never matched the Rust side and would have broken every MCP call on a Linux dev box.
