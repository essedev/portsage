# Portsage - Project conventions

## Overview

A macOS menubar app for managing port allocation across projects. See PROJECT.md for the architecture, DESIGN.md for the design system, ROADMAP.md for the roadmap.

## Commands

- `pnpm tauri dev` - run the app in dev mode
- `pnpm dev` - frontend only (Vite)
- `pnpm build` - build the frontend
- `pnpm tauri build` - build the full app (.dmg)
- `pnpm test` - run TypeScript tests (vitest, one-shot)
- `pnpm test:watch` - vitest watch mode
- `cd src-tauri && cargo test` - run Rust tests

## Structure

```
src/
  components/
    ui/               # Primitives (UICard, UIButton, UISelect, etc.)
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
    lib.rs            # Tauri entry point, tray icon, popover logic
    db.rs             # SQLite setup, migrations, CRUD
    commands.rs       # Tauri commands (frontend-backend IPC)
    scanner.rs        # Port scanner (lsof + ps), unmanaged ports, blocklist
    socket.rs         # Unix socket server for the MCP
  capabilities/
    default.json      # Plugin permissions (dialog, autostart, opener)
  tauri.conf.json     # App config, windows, tray icon, bundle
mcp/
  server.py           # Python MCP server (thin client via stdio)
  SKILL.md            # Claude Code skill file
  install.sh          # Terminal install script
  pyproject.toml      # Python dependencies
homebrew/
  portsage.rb         # Homebrew cask template
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
- All DB access in db.rs, exposed to the frontend via commands.rs
- Port scanning in scanner.rs, do not mix with DB logic
- Unix socket in socket.rs, handles MCP requests
- Database shared via Arc<Database> between Tauri state and the socket server
- Typed errors, no unwrap() in production

### MCP server
- Thin client: no direct DB access
- Talks only via the Unix socket to the Rust backend
- stdio transport for Claude Code integration

### Testing
- Rust tests live inline in each module under `#[cfg(test)] mod tests` - no separate `tests/` tree
- Frontend tests live next to the source as `*.test.ts` and run via vitest
- Both `cargo test` and `pnpm test` must pass before committing or releasing
- Cover behavioral invariants and pure functions, not framework plumbing or thin wrappers - if a test would only verify "framework still works", skip it
- Use in-memory SQLite (`Database::in_memory()`) for db tests, never touch the real config dir
- For race/concurrency fixes, add a regression test that would fail without the fix (see `db.rs::concurrent_create_project_produces_no_overlapping_ranges`)

### General
- Code in English, UI in English (Italian translation tracked separately)
- Line length: 100 characters
- pnpm for the frontend, uv for Python
