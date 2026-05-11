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

The MCP server exposes 14 tools in three groups (full parity with the CLI):

- **Read**: `list_all`, `scan_active`, `list_unmanaged`, `next_range`, `get_config`, `find_project_by_path`
- **Mutate**: `reserve_range`, `register_port`, `remove_port`, `release_project`, `set_config`
- **Act**: `kill_port`, `kill_project`, `open_in_browser`

**Claude Code**: automatic install from the app (Settings > "Configure MCP" > Claude Code) or from terminal (`mcp/install.sh`). Installs the MCP server, the skill file, and tool permissions.

**Other editors**: the app supports copy-paste config for Cursor, Claude Desktop, Cline, VS Code (Copilot), Codex (TOML), Windsurf. The "Other editors" section in Settings > "Configure MCP" shows an editor dropdown plus a pre-generated config with the correct path of the MCP directory, ready to paste into the editor's config file.

The MCP server files (`server.py`, `pyproject.toml`, `SKILL.md`) are bundled as resources in the `.dmg` and copied into `~/Library/Application Support/portsage/mcp/` on first use (see `commands::get_mcp_dir`).

### Socket protocol

The Rust backend listens on a Unix domain socket at `~/Library/Application Support/portsage/portsage.sock` (macOS) / `$XDG_CONFIG_HOME/portsage/portsage.sock` (Linux). The parent directory is created with mode `0700` and the socket file with mode `0600`, so only the current user can connect. Implementation: `src-tauri/src/socket.rs`.

**Transport**: line-delimited JSON over `SOCK_STREAM`. One request per line (`\n` terminated), one response per line. The connection is persistent: a client may send multiple requests on the same connection. Idle connections are closed after 60 seconds.

**Request envelope**:

```json
{ "method": "<name>", "params": { ... } }
```

**Response envelope** is one of:

```json
{ "result": <value> }
{ "error": "<message>" }
```

**Methods** (canonical types defined in `crates/portsage-client/src/types.rs`):

| Method | Params | Result |
| --- | --- | --- |
| `list_all` | none | `[ProjectStatus]` |
| `reserve_range` | `name` (string, required), `path` (string, optional) | `ProjectStatus` |
| `register_port` | `project` (string), `service` (string), `port` (int, must be inside the project range) | `PortStatus` |
| `remove_port` | `project` (string), `service` (string) | `"ok"` |
| `release_project` | `name` (string) | `"ok"` |
| `scan_active` | none | `[ActivePort]` |
| `list_unmanaged` | none | `[ActivePort]` (active ports >= 3000, not registered, filtered by the host blocklist) |
| `next_range` | none | `RangeBounds` |
| `get_config` | none | `ConfigSnapshot` |
| `set_config` | `key` (one of `base_port`, `range_size`), `value` (string) | `"ok"` |
| `kill_port` | `port` (int) | `KillOutcome` |
| `kill_project` | `project` (string) | `[KillEntry]` |
| `open_in_browser` | `port` (int) | `"ok"` |
| `find_project_by_path` | `path` (absolute string) | `ProjectStatus` or `null` |

Type shapes (excerpt, see `crates/portsage-client/src/types.rs` for the source of truth):

- `ProjectStatus`: `{ id, name, path?, range_start, range_end, created_at, ports: [PortStatus] }`
- `PortStatus`: `{ id, project_id, service, port, active, process?, pid?, created_at }`
- `ActivePort`: `{ port, pid, process }`
- `RangeBounds`: `{ range_start, range_end }`
- `ConfigSnapshot`: `{ base_port, range_size }` (string values matching the SQLite TEXT column)
- `KillOutcome`: `"terminated" | "killed" | "not_active" | "permission_denied"`
- `KillEntry`: `{ port, outcome: KillOutcome }`

Errors that can be returned: `invalid json: ...`, `unknown method: ...`, `missing params.<field>`, `project '<name>' not found`, `port <n> is outside project range <a>-<b>`, `set_config: unsupported key '<k>'`, plus any SQLite constraint failure (e.g. duplicate project name, duplicate port).

**Example session** (using `nc -U`):

```text
> {"method":"reserve_range","params":{"name":"myapp","path":"/tmp/myapp"}}
< {"result":{"id":1,"name":"myapp","path":"/tmp/myapp","range_start":4000,"range_end":4009,"created_at":"...","ports":[]}}
> {"method":"register_port","params":{"project":"myapp","service":"vite","port":4000}}
< {"result":{"id":1,"project_id":1,"service":"vite","port":4000,"active":false,"process":null,"pid":null,"created_at":"..."}}
> {"method":"list_all"}
< {"result":[{"id":1,"name":"myapp",...,"ports":[{"id":1,...,"port":4000,"active":false,...}]}]}
```

The Python MCP server (`mcp/server.py`) and the Rust CLI (`crates/portsage-cli`) are both reference clients. Any other language can integrate by speaking this protocol directly.

## Testing strategy

Tests cover the **domain core** - port allocation, lsof parsing, socket protocol, error humanization - and the **safety-critical helpers** (e.g. parse-or-bail before merging into the user's editor config). They do not cover thin wrappers, framework plumbing, or UI rendering.

- Rust tests live inline in each module under `#[cfg(test)] mod tests` and use in-memory SQLite. Run the whole workspace (app + client + cli) with `cargo test` from the repo root.
- Frontend tests live next to the source as `*.test.ts` and run via vitest. Run with `pnpm test`.

The race-condition fix in `Database::create_project` has a dedicated regression test (`concurrent_create_project_produces_no_overlapping_ranges`) that spawns N threads and asserts non-overlapping ranges - it would have failed on the pre-fix code.

## UI

### Menubar popover (quick view)

Compact panel (350x480px), read-only, for fast checks.

- Header with the "portsage" title and an active-ports badge
- List of projects with ports and active state (amber/grey dot)
- Click on any port number opens `http://localhost:PORT` in the default browser
- Footer with port count and "Open portsage" link

### Full app window

Resizable window (min 700x400) with transparent titlebar.

**Header**
- "portsage" title with amber glow and the tagline "ports under control"
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
- Service list: service name, port, active state, process name with PID
- Click on any port number opens `http://localhost:PORT` in the default browser
- Per-port Power button: confirms and sends SIGTERM, escalates to SIGKILL after 2s
- Service add form with a dropdown of free ports in the range
- Actions: remove project, open in Finder, open in Terminal, Stop all active ports (Power button - SIGTERM/SIGKILL on every active port of the project, parallel)

**Unmanaged ports**
- Table with port, process name, PID
- Click on any port number opens `http://localhost:PORT` in the default browser
- Per-row Power button: confirms and sends SIGTERM, escalates to SIGKILL after 2s
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
