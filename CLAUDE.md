# Portsage - Project conventions

## Overview

A macOS menubar app for managing port allocation across projects. See PROJECT.md for the architecture, DESIGN.md for the design system, ROADMAP.md for the roadmap.

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
    ui/               # Primitives (UICard, UIButton, UISelect, UIPortLink, etc.)
    PortRow.tsx
    ProjectCard.tsx
    ProjectDetail.tsx
    ProjectList.tsx
    PopoverPanel.tsx
    Sidebar.tsx
    AppHeader.tsx
    SettingsPanel.tsx
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
    db.rs             # SQLite setup, migrations, CRUD
    actions.rs        # pure logic shared by commands.rs and socket.rs (no Tauri deps)
    commands.rs       # Thin Tauri wrappers over actions::*
    scanner.rs        # Port scanner (lsof + ps), unmanaged ports, blocklist
    socket.rs         # Unix socket server (async), dispatches the wire protocol
  binaries/           # CLI sidecar staged here by scripts/build-cli.sh (gitignored)
  capabilities/
    default.json      # Plugin permissions (dialog, autostart, opener)
  tauri.conf.json     # App config, windows, tray icon, bundle, externalBin
crates/
  portsage-client/    # sync UnixStream client + wire types (single source of truth)
  portsage-cli/       # clap-based binary, talks to the socket via portsage-client
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
- Port scanning in scanner.rs, do not mix with DB logic
- Unix socket in socket.rs, handles MCP and CLI requests
- Database shared via Arc<Database> between Tauri state, the socket server, and the headless runtime
- Typed errors, no unwrap() in production
- Wire types (PortStatus, ProjectStatus, ActivePort, KillOutcome, KillEntry, RangeBounds, ConfigSnapshot) are defined in `crates/portsage-client/src/types.rs` and re-exported from actions.rs and scanner.rs - never duplicate them

### MCP server
- Thin client: no direct DB access
- Talks only via the Unix socket to the Rust backend
- stdio transport for Claude Code integration

### CLI (`crates/portsage-cli`)
- Talks to the backend via `portsage-client` (sync UnixStream); never reads the DB directly
- Auto-spawns the Tauri binary in `--headless` mode if no instance is running (suppressable via `--no-autospawn`)
- Output: human (colored on TTY via anstream/anstyle), `--json`, `-q/--quiet`. Errors go to stderr
- Destructive ops (`release`, `kill`, `kill-project`) refuse to act without `-y` when stdin is not a TTY - piped invocations cannot silently auto-accept
- Exit codes: 0 ok, 1 generic, 2 usage (clap), 3 backend unreachable, 4 not found, 5 conflict

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
