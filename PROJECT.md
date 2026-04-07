# Portsage

> 🇮🇹 [Leggi in italiano](PROJECT.it.md)

A macOS menubar app that manages port allocation across development projects.

## Problem

Working with AI on 4-5 projects in parallel (React/Vite + Docker with PostgreSQL, Redis, S3) constantly causes port collisions. There is no simple way to see which ports are taken, by which project, and which ranges are still free.

## Solution

A macOS app with:
- **Menubar popover**: compact quick view to check the state of your ports
- **Full app window**: full management of projects, ports, and settings
- **MCP server**: Claude Code reserves ports and registers services automatically
- **Unmanaged ports**: detects active ports not associated with any project

## Architecture

### Components

1. **Tauri app** (Tauri v2 + React + Tailwind)
   - Rust backend: sole owner of state, handles DB and port scanning
   - Exposes a local Unix socket (`~/.config/portsage/portsage.sock`) for the MCP server
   - React frontend for popover and full window

2. **MCP server** (Python, thin client)
   - Receives calls from Claude Code via stdio
   - Forwards them to the Tauri backend via Unix socket
   - Does not access the DB directly

3. **Database** (SQLite)
   - Path: `~/.config/portsage/portsage.db`
   - Source of truth, managed exclusively by the Rust backend

### Data flow

```
Claude Code  -->  MCP server (Python/stdio)  -->  Unix socket  -->  Tauri (Rust)  -->  SQLite
                                                                         |
UI (React)  <--  Tauri IPC commands  <------------------------------------
```

### Database schema

```sql
CREATE TABLE projects (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    path TEXT,
    range_start INTEGER NOT NULL,
    range_end INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE ports (
    id INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL REFERENCES projects(id),
    service TEXT NOT NULL,
    port INTEGER NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
-- Defaults: base_port = 4000, range_size = 10
```

### Port scanning

The app scans active ports with `lsof -iTCP -sTCP:LISTEN -nP` and cross-references the DB to show real-time status. Process names are resolved via `ps -p <pid> -o comm=` to avoid lsof's truncation.

Unmanaged ports are filtered: only ports >= 3000, excluding system processes (AirPlay, CUPS, mDNS, Spotlight, etc.).

## Stack

- **Frontend**: React 19 + TypeScript + Tailwind v4
- **App shell**: Tauri v2 (Rust) - menubar, system tray, popover, app window
- **MCP server**: Python + FastMCP SDK (thin client via stdio)
- **Package manager**: pnpm (frontend), uv (Python)
- **Database**: SQLite in `~/.config/portsage/`
- **Font**: system-ui (UI), ui-monospace (titles and technical data)

## Distribution

- **Dev**: `pnpm tauri dev`
- **Build**: `pnpm tauri build` (generates `.dmg`)
- **Homebrew**: `brew tap essedev/portsage && brew install portsage`
- **GitHub**: https://github.com/essedev/portsage

## MCP integration

The MCP server exposes 5 tools:
- `list_all` - full registry plus port status
- `reserve_range(project_name)` - reserves the next free range
- `register_port(project_name, service, port)` - registers a port
- `release_project(project_name)` - releases a range
- `scan_active` - active ports on the machine

**Claude Code**: automatic install from the app (Settings > "Configure MCP" > Claude Code) or from terminal (`mcp/install.sh`). Installs the MCP server, the skill file, and tool permissions.

**Other editors**: the app supports copy-paste config for Cursor, Claude Desktop, Cline, VS Code (Copilot), Codex (TOML), Windsurf. The "Other editors" section in Settings > "Configure MCP" shows an editor dropdown plus a pre-generated config with the correct path of the MCP directory, ready to paste into the editor's config file.

The MCP server files (`server.py`, `pyproject.toml`, `SKILL.md`) are bundled as resources in the `.dmg` and copied into `~/Library/Application Support/portsage/mcp/` on first use (see `commands::get_mcp_dir`).

## UI

### Menubar popover (quick view)

Compact panel (350x480px), read-only, for fast checks.

- Header with the "portsage" title and an active-ports badge
- List of projects with ports and active state (amber/grey dot)
- Footer with port count and "Open portsage" link

### Full app window

Resizable window (min 700x400) with transparent titlebar.

**Header**
- "portsage" title with amber glow and the tagline "your port sage"
- Active ports badge

**Sidebar**
- Search/filter input
- "New project" button
- Project list with active-ports badges
- "Unmanaged" section with count (visible only if any are present)
- "Configure MCP" button (visible only if MCP is not installed for Claude Code)
- "Settings" button

**Project detail** (main pane)
- Name, path, assigned range, active-ports badge
- Service list: service name, port, active state, process name
- Service add form with a dropdown of free ports in the range
- Actions: remove project, open in Finder, open in Terminal

**Unmanaged ports**
- Table with port, process name, PID
- Only ports >= 3000, system processes filtered out

**Settings**
- Launch at login (toggle)
- Configure base_port and range_size
- Configure MCP: two sections
  - **Claude Code**: connection state, install/remove, list of available tools
  - **Other editors**: editor dropdown plus copy-paste config with instructions for Cursor, Windsurf, VS Code, Claude Desktop, Continue, Cline, Codex, Zed
- Data export/import (.portsage = zip with DB + config)

### Menubar icon

- Template icon (adapts to macOS dark/light mode)
