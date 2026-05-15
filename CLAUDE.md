# Portsage - Project conventions

## Overview

A macOS menubar app for managing port allocation across projects. See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the architecture, [docs/DATABASE_SCHEMA.md](docs/DATABASE_SCHEMA.md) for the SQLite schema, [docs/DESIGN.md](docs/DESIGN.md) for the design system, [docs/ROADMAP.md](docs/ROADMAP.md) for the roadmap, [docs/RELEASING.md](docs/RELEASING.md) for the release process.

## Active evolution

A multi-host evolution is in flight (Linux headless server, remote backends in the UI, SSH auto-forward). The detailed plan is in [docs/multi-host-evolution.md](docs/multi-host-evolution.md). The ROADMAP entry is v0.8.

Phase 1 (Linux headless server), Phase 2 (remote backend in the UI), and Phase 3 (auto SSH port forwarding) are all shipped and validated end-to-end against the `forge` dev box on 2026-05-12. Two live-smoke-test bugs were caught + fixed during validation: the socket-file 0600 mode that blocked `portsage` group access through systemd's `RuntimeDirectory=0750` (commit `87b0334`), and the systemd `User=portsage Group=portsage` default that broke `/proc/<pid>/fd` readlink for processes owned by the dev user because the kernel's `__ptrace_may_access(PTRACE_MODE_FSCREDS)` requires both `fsuid` AND `fsgid` to match the target's creds (commit `d428c47`). Phase 3 deferred items: server-pushed `StateChanged` events (60s periodic sync + post-mutation sync covers the lag, fine for the dev-server use case), macOS notifications on local port conflict (failure surfaces in the indicator tooltip + toast). Phase 4 (polish) is not yet started.

Read the plan before touching any of these modules - they were designed against it:
- `scanner.rs`: per-OS implementations under `mod macos` / `mod linux`, selected by `#[cfg(target_os)]`. Wire type `ActivePort` lives in `portsage-client/types.rs`.
- `backends.rs`: `BackendManager` owns SSH tunnels; `BackendRouter` owns the active target; `BackendClient` is the Local/Remote adapter every Tauri command dispatches through.
- `forwards.rs`: `ForwardManager` owns per-(backend, port) SSH local-forward state. Two ControlMaster ownership modes: piggyback on the user's `ssh_config` `ControlMaster auto` (preferred; never tear it down), or open a Portsage-managed master at `paths::state_dir()/cm-<alias>.sock` (tracked + closed on app quit). Tests use the `ForwardController` + `LocalPortProbe` traits.
- `db.rs`: `remote_backends` table is additive; the row type re-exports `portsage_client::RemoteBackend` so the wire and on-disk shapes can't drift. `forward_exclusions` table is per-(backend, port) with UNIQUE and cascade on backend deletion in code.

When changes break SSH on `forge`, check `pgrep -af sshd` first - we permanently disabled Tailscale SSH on the box so the unix-socket forward path works; if a Tailscale ACL push ever re-enables it, every `-L unix:...` silently breaks. Memory: `forge-server.md` carries the decision + reason.

## Commands

- `pnpm tauri dev` - run the app in dev mode
- `pnpm dev` - frontend only (Vite)
- `pnpm build` - build the frontend
- `pnpm build:cli` - build the CLI and stage it for Tauri's externalBin bundler
- `pnpm tauri build` - build the full app (.dmg). Chains `pnpm build && pnpm build:cli` via `beforeBuildCommand` so the CLI sidecar is staged before bundling
- `pnpm test` - run TypeScript tests (vitest, one-shot)
- `pnpm test:watch` - vitest watch mode
- `cargo test` - run all workspace Rust tests (app + portsage-client + portsage-cli)
- `cargo run -p portsage-cli -- <args>` - invoke the CLI from a dev build

## Structure

```
src/
  components/
    ui/                    # Primitives (UICard, UIButton, UISelect, UIPortLink, etc.)
    PortRow.tsx
    ProjectDetail.tsx
    PopoverPanel.tsx
    Sidebar.tsx
    BackendSwitcher.tsx    # Sidebar dropdown for Local / Remote backends, live tunnel state dot
    AppHeader.tsx
    WelcomePanel.tsx       # Empty-state main window (first-run CTA + stat cards when projects exist)
    SettingsPanel.tsx
    RemoteBackendsPanel.tsx # Settings tab: remote backend CRUD, auto-forward toggle, excluded ports
    UnmanagedPortsPanel.tsx
    AddProjectForm.tsx
    AddPortForm.tsx
  features/
    projects/         # useProjects hook
  lib/
    commands.ts       # Tauri invoke command wrappers
    types.ts          # TypeScript types (ProjectStatus, PortStatus, etc.)
  App.tsx             # Routing for main window vs popover
  main.tsx
  index.css           # CSS custom tokens (@theme Tailwind v4)
src-tauri/
  src/
    main.rs           # entry: dispatches to GUI or --headless mode based on argv
    lib.rs            # Tauri entry point, tray icon, popover logic, run_headless()
    db.rs             # SQLite setup, migrations, CRUD (projects, ports, remote_backends)
    paths.rs          # OS-aware path resolution (XDG on Linux, Application Support on macOS)
    actions.rs        # pure logic shared by commands.rs and socket.rs (no Tauri deps)
    commands.rs       # Thin Tauri wrappers over actions::* and backends::*
    scanner.rs        # Port scanner (macOS lsof + ps, Linux /proc + ss fallback)
    socket.rs         # Unix socket server (async), dispatches the wire protocol
    backends.rs       # Multi-host: BackendTarget, BackendManager, SshTunnel, BackendRouter, BackendClient (no Tauri deps)
    forwards.rs       # Phase 3 multi-host: ForwardManager, ForwardController, local-port collision probe, ControlMaster ownership (no Tauri deps)
  binaries/           # CLI sidecar staged here by scripts/build-cli.sh (gitignored)
  capabilities/
    default.json      # Plugin permissions (dialog, autostart, opener)
  tauri.conf.json     # App config, windows, tray icon, bundle, externalBin
crates/
  portsage-client/    # sync UnixStream client + wire types (single source of truth)
  portsage-cli/       # clap-based binary, talks to the socket via portsage-client
  portsage-mcp/       # shared MCP-install logic (parse-or-bail JSON edit + atomic-tmp-then-rename),
                      # consumed by both `portsage mcp install` (CLI) and the Tauri Settings panel
mcp/
  server.py           # Python MCP server (thin client via stdio)
  SKILL.md            # Claude Code skill file
  install.sh          # Terminal install script
  pyproject.toml      # Python dependencies
homebrew/
  portsage.rb         # Homebrew cask template
scripts/
  build-cli.sh        # Builds portsage-cli and stages it for externalBin
```

## Rules

### Frontend
- All colors via the CSS tokens defined in DESIGN.md, never hardcoded
- Spacing only via the --space-N tokens, border-radius via --radius-*
- `UI*` components in `src/components/ui/`, composed in `src/components/`
- One-way dependency: primitives <- composed. `UI*` never imports composed
- Props typed with an interface in the same component file
- Tailwind v4: CSS-first config with @theme, no tailwind.config.ts
- Font: system-ui (UI), ui-monospace (titles/technical data)
- Import alias: `@/` for absolute imports
- Custom dropdown (UISelect), never the native select

### Rust backend
- All DB access in db.rs, exposed to the frontend via commands.rs and to socket clients via socket.rs
- Shared logic between Tauri commands and the socket dispatcher lives in actions.rs - no Tauri deps allowed there
- Multi-host plumbing (BackendTarget, BackendManager, SshTunnel, BackendRouter, BackendClient) lives in backends.rs - no Tauri deps allowed there either; commands.rs is the only Tauri layer
- OS-aware paths in paths.rs (XDG on Linux, Application Support on macOS) - never call `dirs::*` outside of this module
- Port scanning in scanner.rs, do not mix with DB logic. Per-OS implementations under `mod macos` / `mod linux` selected by `#[cfg(target_os)]`
- Unix socket in socket.rs, handles MCP and CLI requests
- Database shared via Arc<Database> between Tauri state, the socket server, and the headless runtime
- Tauri code is feature-gated behind the `gui` feature (default). The Linux server build runs with `--no-default-features` and drops the entire Tauri toolchain
- Typed errors, no unwrap() in production
- Wire types (PortStatus, ProjectStatus, ActivePort, KillOutcome, KillEntry, RangeBounds, ConfigSnapshot) are defined in `crates/portsage-client/src/types.rs` and re-exported from actions.rs and scanner.rs - never duplicate them

### MCP server
- Thin client: no direct DB access
- Talks only via the Unix socket to the Rust backend
- stdio transport for Claude Code integration
- Socket path resolution mirrors `portsage_client::default_socket_path` (env override > macOS Application Support > Linux `$XDG_RUNTIME_DIR` with `/tmp/portsage-<uid>.sock` fallback). The `PORTSAGE_SOCKET` env var lets the MCP point at a system-wide systemd socket or a forwarded one
- The four source files (`server.py`, `pyproject.toml`, `uv.lock`, `SKILL.md`) are embedded into the CLI binary via `include_str!` in `crates/portsage-cli/src/mcp.rs`. `portsage mcp install` extracts them to `<data_dir>/portsage/mcp/` (Linux: `~/.local/share/portsage/mcp/`, macOS: `~/Library/Application Support/portsage/mcp/`). The `mcp/` directory in the repo remains the source of truth - any changes there must also rebuild the CLI before they're picked up

### CLI (`crates/portsage-cli`)
- Talks to the backend via `portsage-client` (sync UnixStream); never reads the DB directly
- Auto-spawns the Tauri binary in `--headless` mode if no instance is running (suppressable via `--no-autospawn`)
- Output: human (colored on TTY via anstream/anstyle), `--json`, `-q/--quiet`. Errors go to stderr
- Destructive ops (`release`, `kill`, `kill-project`) refuse to act without `-y` when stdin is not a TTY - piped invocations cannot silently auto-accept
- Exit codes: 0 ok, 1 generic, 2 usage (clap), 3 backend unreachable, 4 not found, 5 conflict
- `portsage mcp install/uninstall/status` manages the Claude Code MCP integration without needing the backend running - the embedded files are written, `uv sync` runs, and `~/.claude.json` / `~/.claude/skills/portsage/` / `~/.claude/settings.json` are patched atomically. The same JSON-merge safety (parse-or-bail rather than clobber) used by `install_mcp` in `src-tauri/commands.rs` applies in `crates/portsage-cli/src/mcp.rs`
- `portsage self-update` shells out to `curl` to fetch the latest GitHub release tag, compares against `env!("CARGO_PKG_VERSION")`, and on macOS with brew available runs `brew update && brew upgrade --cask portsage`. On Linux it prints the release URL rather than overwriting a running binary held open by systemd

### Headless mode (`--headless` / `-H`)
- Same binary as the GUI; argv detection in `main.rs` dispatches to `run_headless()` before any Tauri state is constructed
- Probes the socket first - if another instance is already serving, exits cleanly without clobbering the socket file
- Blocks on SIGINT or SIGTERM for clean shutdown (avoids stale sockets after `brew upgrade`)

### Testing
- Rust tests live inline in each module under `#[cfg(test)] mod tests` - no separate `tests/` tree
- Frontend tests live next to the source as `*.test.ts` and run via vitest
- Both `cargo test` (workspace, all three crates) and `pnpm test` must pass before committing or releasing
- Cover behavioral invariants and pure functions, not framework plumbing or thin wrappers - if a test would only verify "framework still works", skip it
- Use in-memory SQLite (`Database::in_memory()`) for db tests, never touch the real config dir
- For race/concurrency fixes, add a regression test that would fail without the fix (see `db.rs::concurrent_create_project_produces_no_overlapping_ranges`)
- For the socket protocol, prefer end-to-end tests that spawn a real `UnixListener` and exercise `portsage_client::Client` against `handle_request` (see `socket.rs::end_to_end_round_trip_via_real_client`) - this catches drift between the client's deserialization target and the server's response shape

### General
- Code in English, UI in English (Italian translation tracked separately)
- Line length: 100 characters
- pnpm for the frontend, uv for Python
