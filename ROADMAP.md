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

See `FEATURES_TODO.md` for details on each.

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

The full plan lives in [`docs/multi-host-evolution.md`](docs/multi-host-evolution.md). Shipped in 4 phases:

- **Phase 1 - Cross-platform server** (code complete, awaits a real Linux smoke test): Linux x86_64 headless build (`portsage-server`), scanner abstraction (macOS lsof vs Linux `/proc/net/tcp`), XDG paths, systemd unit. Unblocks running portsage on dedicated dev servers and lets remote Claude Code agents talk to a local portsage MCP. Done: scanner abstraction with macOS / Linux impls, XDG path module, `gui` cargo feature gating Tauri so Linux can build headless-only, `--socket` override + `PORTSAGE_SOCKET` env, systemd unit at `packaging/linux/portsage-server.service`, idempotent `packaging/linux/install.sh`, CI workflows. Remaining: real test run on Linux x86_64 (the build pipeline does this on tag), Homebrew-on-Linux integration (low priority, see plan #4.x).
- **Phase 2 - Remote backend in the UI** (Rust+CLI validated on forge, Tauri UI portion built but not yet exercised in a running window): Mac UI can configure remote Portsage servers, open SSH local-socket tunnels to them, and run every project/port command against the selected backend. Done: `remote_backends` schema + CRUD, `BackendManager` + `SshTunnel` with state machine and per-backend mutex, `BackendRouter` + `BackendClient` adapter, 10 Tauri commands (CRUD + test + tunnel statuses + current target persistence), `BackendSwitcher` in the sidebar with live status dot via `tunnel://state-changed` events, "Remote backends" tab in Settings with add/test/remove/auto-forward toggle, all existing Tauri commands routed through the active backend, `ProjectDetail` hides Finder/Terminal buttons for Remote, CLI `--backend <name>` flag with `PORTSAGE_BACKEND` env (delegates tunnel lifecycle to the Mac app rather than opening its own), `humanizeError` covers SSH-specific failure modes. Validated 2026-05-12: dev server `forge` running the musl tarball under systemd, Mac CLI talks through `ssh -L unix:/tmp/portsage-forge.sock:/run/portsage/portsage.sock` and round-trips reserve/register/list/release. Live bug found during smoke test: socket file was 0600 inside systemd's 0750 RuntimeDirectory, blocking group access; fix in commit 87b0334 picks 0660 when the parent dir is externally managed. Divergence from plan: the CLI does not open tunnels itself, it reads the local-side forwarded socket path from the Mac app's socket and connects to that - the plan section 2.5 said "opens a tunnel just like the UI does", but cross-process tunnel state would mean two `BackendManager` instances racing on the same SSH child.
- **Phase 3 - SSH port forwarding integration** (MVP shipped, polish pending): Mac UI exposes per-port forward toggle, `ForwardManager` owns the (backend, port) -> state map, the `ForwardController` trait shells out to `ssh -O forward / cancel` and gracefully handles "user has ControlMaster" vs "we need to open one ourselves" (managed masters live at `paths::state_dir()/cm-<alias>.sock` and get torn down on app quit). Local-port collision probe surfaces "port X is in use locally by node (pid 12345)" before issuing the ssh call, instead of letting the bind silently fail. Three port-row states render with an arrow icon: active+forwarded (amber), failed (red, hover for reason), cancelled (muted, click to re-open). `forward_exclusions` table persists per-backend port blocklists; the underlying Tauri commands are wired (`list_forward_exclusions`, `add_forward_exclusion`, `remove_forward_exclusion`) but no UI in Settings yet. Known gaps before declaring Phase 3 complete: (a) no startup auto-sync for backends with `auto_forward_enabled = true`, (b) no periodic 60s safety-net timer, (c) no server-push `StateChanged` event channel (manual sync only - the agent on the remote registering a port from MCP doesn't propagate to the Mac until the user clicks something), (d) no macOS notifications on local port conflict, (e) no excluded-ports sub-UI in Settings > Remote backends.
- **Phase 2 - Remote backend in the UI**: macOS menubar app gains a backend switcher (`Local` / `Remote: dev`). Connects via SSH unix-socket forwarding (`ssh -L unix:...`). New `remote_backends` table, full project CRUD against remote backends.
- **Phase 3 - SSH port forwarding integration**: when a remote project has registered ports, Portsage opens SSH local forwards automatically via `ssh -O forward` over a ControlMaster session. Tracks lifecycle, detects local port collisions, surfaces conflicts in the UI.
- **Phase 4 - Quality of life**: project migration between backends, health dashboard, CLI `--backend <name>` flag, Tailscale host auto-discovery.

Effort estimate: 2-3 weeks of focused work, shippable incrementally.

This roadmap entry subsumes the Linux support that was listed in v0.7. Windows support remains in v0.7 as a separate concern.
