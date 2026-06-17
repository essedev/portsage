# Portsage - Architecture

> 🇮🇹 [Leggi in italiano](ARCHITECTURE.it.md)

A menubar app that manages port allocation across development projects, with a Linux headless server variant and a multi-host UI mode for remote development boxes.

## Problem

Working with AI on 4-5 projects in parallel (React/Vite + Docker with PostgreSQL, Redis, S3) constantly causes port collisions. There is no simple way to see which ports are taken, by which project, and which ranges are still free. The problem gets worse when projects live on a remote Linux box reached over SSH.

## Components

```
+-----------------------------------------------------------------+
|  macOS app (Tauri v2 + React)                                   |
|  +---------------+    +---------------------+   +------------+  |
|  | Tray + popover|    | Full window (React) |   | Settings   |  |
|  +-------+-------+    +----------+----------+   +-----+------+  |
|          +-------------- IPC ----+---------------------+        |
|                              |                                  |
|  +---------------------------v------------------------------+   |
|  | Rust backend                                             |   |
|  |   db.rs / actions.rs / commands.rs / socket.rs           |   |
|  |   scanner.rs (per-OS) / backends.rs / forwards.rs        |   |
|  +-----+-----------------------+----------------------------+   |
|        |                       |                                |
+--------+-----------------------+--------------------------------+
         |                       |
   +-----v-----+           +-----v--------------+
   | Unix sock |<--- MCP --| Python MCP server  |
   | (local)   |   CLI ----| Rust CLI (portsage)|
   +-----+-----+           +--------------------+
         |
         +-- BackendClient --> Local SQLite (default)
                          \-> SSH unix-socket tunnel --> Remote portsage-server (Linux)
```

The same Rust binary runs as the macOS GUI (`gui` cargo feature, default) and as the Linux headless server (`--no-default-features`, drops the Tauri toolchain entirely). On macOS the `--headless` flag suppresses the tray + windows and exposes only the socket; this is how the CLI auto-spawns the backend when no GUI instance is running.

### Rust backend modules

| Module        | Responsibility                                                             |
|---------------|----------------------------------------------------------------------------|
| `paths.rs`    | OS-aware path resolution (Application Support on macOS, XDG on Linux). The only place `dirs::*` is allowed. |
| `db.rs`       | SQLite setup, migrations, all CRUD. Shared via `Arc<Database>`. See [DATABASE_SCHEMA.md](DATABASE_SCHEMA.md). |
| `actions.rs`  | Pure business logic shared between Tauri commands and the socket dispatcher. No Tauri deps. |
| `commands.rs` | Thin Tauri wrappers over `actions::*` and `backends::*`. The only Tauri layer. |
| `socket.rs`   | Unix socket server (async). Speaks the wire protocol to the MCP server, the CLI, and other clients. |
| `scanner.rs`  | Port scanner. Per-OS impls under `mod macos` (lsof + ps) and `mod linux` (`/proc/net/tcp` + `ss` fallback), selected by `#[cfg(target_os)]`. |
| `backends.rs` | `BackendTarget` / `BackendManager` (owns SSH tunnels) / `BackendRouter` (active target) / `BackendClient` (Local/Remote adapter every Tauri command dispatches through). No Tauri deps. |
| `forwards.rs` | Phase 3 multi-host: `ForwardManager` owns per-(backend, port) SSH local-forward state. `ForwardController` + `LocalPortProbe` traits for testability. No Tauri deps. |

### MCP server

Thin Python client. Reads from stdio (Claude Code transport), forwards JSON requests to the Unix socket, returns the response. No direct DB access.

The four source files (`server.py`, `pyproject.toml`, `uv.lock`, `SKILL.md`) are embedded into the CLI binary via `include_str!` in `crates/portsage-cli/src/mcp.rs`. `portsage mcp install` extracts them to `<data_dir>/portsage/mcp/`, runs `uv sync`, and patches `~/.claude.json` + `~/.claude/skills/portsage/` + `~/.claude/settings.json` atomically. The `mcp/` directory in the repo remains the source of truth - any change there must rebuild the CLI before it ships.

### CLI

`portsage` is a clap-based binary in `crates/portsage-cli/`. Talks to the backend via `portsage-client` (sync UnixStream), never reads the DB directly. Auto-spawns the Tauri binary in `--headless` mode if no instance is running.

See the [README](../README.md#cli) for the full subcommand surface.

## File locations

| Item     | macOS                                                  | Linux                                                           |
|----------|--------------------------------------------------------|-----------------------------------------------------------------|
| Database | `~/Library/Application Support/portsage/portsage.db`   | `$XDG_DATA_HOME/portsage/portsage.db` (default `~/.local/share/portsage/`) |
| Socket   | `~/Library/Application Support/portsage/portsage.sock` | `$XDG_RUNTIME_DIR/portsage/portsage.sock` (fallback `/tmp/portsage-<uid>.sock`) |
| MCP dir  | `~/Library/Application Support/portsage/mcp/`          | `~/.local/share/portsage/mcp/`                                  |
| State    | same as Application Support                            | `$XDG_STATE_HOME/portsage/` (default `~/.local/state/portsage/`) |

The headless server accepts `--socket <path>` and `PORTSAGE_SOCKET=<path>` to override the socket location; the system-wide systemd unit uses this to place the socket at `/run/portsage/portsage.sock`.

The parent directory of the socket is created with mode `0700`, the socket file with `0600` for per-user installs and `0660` when the parent dir is externally managed (systemd-style shared group install) - see `socket.rs` for the picker.

## Multi-host

The single-host story (Mac UI talks to a local backend) is the default. The multi-host extension lets the Mac UI configure remote Portsage servers and treat them as first-class backends.

- **Phase 1 - Linux headless server.** Linux x86_64 build (`portsage-server`), per-OS scanner, XDG paths, systemd unit + idempotent installer in `packaging/linux/`.
- **Phase 2 - Remote backend in the UI.** `remote_backends` table, `BackendManager` owns per-backend SSH unix-socket tunnels with a state machine and per-backend mutex, `BackendRouter` with persisted current target. Every Tauri command dispatches through the active `BackendClient`. The sidebar `BackendSwitcher` shows a live state dot via `tunnel://state-changed` events.
- **Phase 3 - Auto SSH port forwarding.** When a remote backend has registered ports, Portsage opens `ssh -O forward -L <port>:localhost:<port>` against the same ControlMaster the protocol tunnel uses. Two ControlMaster ownership modes: piggyback on the user's `ssh_config` `ControlMaster auto` (preferred; never tear it down), or open a Portsage-managed master at `paths::state_dir()/cm-<alias>.sock`. Local-port collision probe surfaces "port X is in use locally by node (pid 12345)" before issuing the ssh call. A 60s periodic sync timer is the safety-net for remote-driven mutations.
- **Phase 4 - Polish.** Project migration between backends, health dashboard, CLI `portsage backends list / add / remove`, Tailscale host auto-discovery. Not started yet.

The full plan + design decisions live in [multi-host-evolution.md](multi-host-evolution.md). Read it before touching `scanner.rs`, `backends.rs`, `forwards.rs`, or the multi-host bits of `db.rs`.

## Socket protocol

The Rust backend listens on a Unix domain socket (see the table above for the path).

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

| Method                 | Params                                                                | Result               |
|------------------------|-----------------------------------------------------------------------|----------------------|
| `list_all`             | none                                                                  | `[ProjectStatus]`    |
| `reserve_range`        | `name` (string, required), `path` (string, optional)                  | `ProjectStatus`      |
| `update_project`       | `current_name` (string), `new_name` (string, optional), `new_path` (string, optional; empty clears the path) | `ProjectStatus` |
| `register_port`        | `project` (string), `service` (string), `port` (int, inside range)    | `PortStatus`         |
| `remove_port`          | `project` (string), `service` (string)                                | `"ok"`               |
| `release_project`      | `name` (string)                                                       | `"ok"`               |
| `scan_active`          | none                                                                  | `[ActivePort]`       |
| `list_unmanaged`       | none                                                                  | `[ActivePort]` (active ports >= 3000, not registered, host blocklist applied) |
| `next_range`           | none                                                                  | `RangeBounds`        |
| `get_config`           | none                                                                  | `ConfigSnapshot`     |
| `set_config`           | `key` (one of `base_port`, `range_size`), `value` (string)            | `"ok"`               |
| `kill_port`            | `port` (int)                                                          | `KillOutcome`        |
| `kill_project`         | `project` (string)                                                    | `[KillEntry]`        |
| `open_in_browser`      | `port` (int)                                                          | `"ok"`               |
| `find_project_by_path` | `path` (absolute string)                                              | `ProjectStatus` or `null` |

Wire types (`crates/portsage-client/src/types.rs` is the source of truth):

- `ProjectStatus`: `{ id, name, path?, range_start, range_end, created_at, ports: [PortStatus] }`
- `PortStatus`: `{ id, project_id, service, port, active, process?, pid?, created_at }`
- `ActivePort`: `{ port, pid, process }`
- `RangeBounds`: `{ range_start, range_end }`
- `ConfigSnapshot`: `{ base_port, range_size }` (string values matching the SQLite TEXT column)
- `KillOutcome`: `"terminated" | "killed" | "not_active" | "permission_denied"`
- `KillEntry`: `{ port, outcome: KillOutcome }`
- `RemoteBackend`: `{ id, name, ssh_alias, remote_socket_path, local_socket_path, auto_forward_enabled, created_at }`

Errors that can be returned: `invalid json: ...`, `unknown method: ...`, `missing params.<field>`, `project '<name>' not found`, `port <n> is outside project range <a>-<b>`, `set_config: unsupported key '<k>'`, plus any SQLite constraint failure (e.g. duplicate project name, duplicate port). SSH-specific failures surface verbatim - the frontend's `humanizeError` maps the common ones.

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

## Database

See [DATABASE_SCHEMA.md](DATABASE_SCHEMA.md) for the full table-by-table breakdown and the path resolution.

## Port scanning

- **macOS**: `lsof -iTCP -sTCP:LISTEN -nP`, process names resolved via `ps -p <pid> -o comm=` to bypass lsof's truncation.
- **Linux**: parses `/proc/net/tcp` (+ `/proc/net/tcp6`) and resolves the owning pid via `/proc/<pid>/fd/*` readlinks; falls back to `ss -tlnp` if `/proc` is restricted (e.g. inside a container without `--cap-add SYS_PTRACE`).

Unmanaged ports are filtered to ports >= 3000, excluding well-known system processes (AirPlay, CUPS, mDNS, Spotlight, sshd, etc.). The blocklist is in `scanner.rs::is_system_process`.

## Stack

- **Frontend**: React 19 + TypeScript + Tailwind v4 (CSS-first via `@theme`).
- **App shell**: Tauri v2 (Rust) - menubar, system tray, popover, app window.
- **MCP server**: Python + FastMCP SDK (thin client via stdio).
- **CLI**: Rust + clap, sync UnixStream client.
- **Database**: SQLite (see [DATABASE_SCHEMA.md](DATABASE_SCHEMA.md)).
- **Package manager**: pnpm (frontend), uv (Python), cargo (Rust workspace).
- **Font**: system-ui (UI), ui-monospace (titles and technical data).

## Distribution

- **Dev**: `pnpm tauri dev`.
- **Build**: `pnpm tauri build` (generates `.dmg`).
- **Homebrew**: `brew tap essedev/portsage && brew install portsage`.
- **Linux server**: tarball from the GitHub release; `packaging/linux/install.sh` installs the binary + systemd unit, rewriting `User=`/`Group=` to the target dev user (the kernel's `__ptrace_may_access(PTRACE_MODE_FSCREDS)` requires `fsuid` AND `fsgid` to match for `/proc/<pid>/fd` readlinks).
- **Release process**: see [RELEASING.md](RELEASING.md).
- **GitHub**: <https://github.com/essedev/portsage>.

## MCP integration

The MCP server exposes 15 tools in three groups (full parity with the CLI):

- **Read**: `list_all`, `scan_active`, `list_unmanaged`, `next_range`, `get_config`, `find_project_by_path`.
- **Mutate**: `reserve_range`, `update_project`, `register_port`, `remove_port`, `release_project`, `set_config`.
- **Act**: `kill_port`, `kill_project`, `open_in_browser`.

**Claude Code**: install via `portsage mcp install` (canonical, works without the GUI running) or from the app (Settings > "Configure MCP" > Claude Code). `portsage mcp uninstall` and `portsage mcp status` complete the lifecycle. Patches to `~/.claude.json` / `~/.claude/skills/portsage/` / `~/.claude/settings.json` go through a parse-or-bail + atomic-tmp-then-rename helper - a corrupt config is never silently overwritten.

**Other editors**: the app generates ready-to-paste config for Cursor, Claude Desktop, Cline, VS Code (Copilot), Codex (TOML), Windsurf. Settings > "Configure MCP" > "Other editors" shows an editor dropdown plus the generated snippet.

## Testing strategy

Tests cover the **domain core** - port allocation, lsof / `/proc/net/tcp` parsing, socket protocol, error humanisation, JSON-merge safety for editor configs - and the **safety-critical helpers**. They do not cover thin wrappers, framework plumbing, or UI rendering.

- Rust tests live inline under `#[cfg(test)] mod tests` in each module; use in-memory SQLite via `Database::in_memory()`. Run the workspace with `cargo test` from the repo root.
- Frontend tests live next to the source as `*.test.ts` and run via `vitest`.
- Socket protocol: prefer end-to-end tests that spawn a real `UnixListener` and exercise `portsage_client::Client` against `handle_request` (see `socket.rs::end_to_end_round_trip_via_real_client`) - this catches drift between the client deserialiser and the server response shape.
- Race-condition fix in `Database::create_project` has its own regression test (`concurrent_create_project_produces_no_overlapping_ranges`).

## UI

### Menubar popover

Compact panel (350x480px), read-only, for fast checks.

- Header: "portsage" title with amber glow + active-ports badge + Quit icon.
- List of projects with ports and active state (amber/grey dot).
- Click on any port number opens `http://localhost:PORT` in the default browser.
- Footer with port count and "Open portsage" link.

### Full window

Resizable (min 700x400) with transparent titlebar.

**Header**
- "portsage" title with amber glow and the tagline "ports under control".
- Active-ports badge.

**Sidebar**
- `BackendSwitcher` dropdown (Local / Remote ssh aliases) with a live state dot.
- Search/filter input.
- "New project" button.
- Project list with active-ports badges (active project name in `text-primary`, inactive in `text-secondary`).
- "Unmanaged" section (count, visible only if any are present).
- "Configure MCP" button (visible only if MCP is not installed for Claude Code).
- "Settings" button.

**Welcome panel** (no selection)
- First run: tagline + "Create your first project" CTA + "Next steps" checklist with deep-link to Settings > Integrations.
- With projects: stat cards (Projects / Registered ports / Active) + quick buttons.

**Project detail**
- Name, path, assigned range, active-ports badge.
- Service list: service name, process, PID, port, active state.
- Click on any port number opens `http://localhost:PORT` in the default browser.
- Per-port Power button (warning/amber): confirms and sends SIGTERM, escalates to SIGKILL after 2s.
- Service add form with a dropdown of free ports in the range.
- Toolbar split: navigation actions (Finder / Terminal - hidden when the active backend is Remote) on the left, destructive actions (Stop all active ports / Delete project) on the right.
- Phase 3 forward indicators: arrow next to each remote port - amber (forwarded), red (failed, hover for reason), muted (cancelled, click to re-open).

**Unmanaged ports**
- Table with port, process name, PID.
- Click on any port number opens `http://localhost:PORT` in the default browser.
- Per-row Power button (warning/amber).
- Only ports >= 3000, system processes filtered out.

**Settings** (tabs)
- **General**: launch at login, base_port + range_size.
- **Integrations**: Claude Code MCP (state, install/remove, tool list) + Other editors (accordion, closed by default).
- **Remote backends**: list, add/test/remove, auto-forward toggle, per-backend "Excluded ports" sub-UI.
- **Data**: export/import (`.portsage` = zip with DB + config).

### Menubar icon

Template icon (adapts to macOS dark/light mode).

## Conventions

Project-specific conventions, allowed/forbidden imports, line length, and the "what lives where" map are in the root [CLAUDE.md](../CLAUDE.md). Design tokens, typography, spacing, and the component library are in [DESIGN.md](DESIGN.md).
