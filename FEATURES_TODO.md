# Features TODO

Features planned to reach parity with existing competitors (Portkeeper, Port Manager, Port Killer) and beyond. Each one is self-contained and can be implemented independently.

---

## 1. Kill process from the UI - DONE

Shipped: per-port Power button on PortRow and on unmanaged port rows, per-project Power button in the project detail header. Sends `kill -TERM <pid>`, waits 2s, escalates to `kill -KILL <pid>` if the process is still alive. Confirmation modal lists the targeted processes and PIDs; toasts surface the outcome (`terminated`, `killed`, `not_active`, `permission_denied`). PID is fetched from a fresh `lsof` scan at kill time to avoid stale data, and a refresh runs immediately after.

Tauri commands: `kill_port(port: i64) -> KillOutcome` and `kill_project(project_id: i64) -> [(port, outcome)]`, both async. Project-level kills run concurrently via `tokio::spawn`.

Not shipped (still open):
- MCP tool `kill_port(port: int)` so Claude can kill zombies on request
- A global "kill all" was considered and explicitly skipped: ambiguous scope, high blast radius

---

## 2. Open in browser - DONE

Shipped: click on any port number opens `http://localhost:PORT` in the default browser. Available in main window project detail, popover, and unmanaged ports panel. Backed by `open_in_browser(port: i64)` (runs macOS `open <url>`) and a `UIPortLink` primitive. No HTTP heuristic - we let the browser report "connection refused" if the port isn't actually HTTP, which is clearer than a heuristic blacklist.

The original "Copy URL to clipboard" sub-item was dropped: the URL is always `http://localhost:<port>`, the port number is already visible in every surface, and the user can type `localhost:4000` directly into Slack/curl/etc. faster than reaching for a copy affordance. The feature would only earn its place if Portsage tracked non-trivial URLs (paths, query strings, non-localhost hosts) - which it doesn't, by design.

---

## 3. CLI - DONE

Shipped: a `portsage` binary in Rust, bundled inside `Portsage.app` and exposed on PATH via the Homebrew cask. Talks to the Tauri backend over the same Unix socket as the MCP server, auto-spawning the backend in `--headless` mode (no tray, no windows, socket only) when no instance is running.

Subcommands (full parity with the MCP surface): `list` (`--here`, `--project`, `--active`), `status`, `reserve` (`--here`, `--path`), `register` (`--here`, `--project`), `remove`, `release`, `scan` (`--unmanaged`), `kill`, `kill-project`, `open`, `config get|set`, `doctor`. Destructive ops require interactive confirmation or `-y`; piped invocations without `-y` refuse to act so they cannot silently auto-accept.

Output modes: human (colored on TTY via anstream/anstyle), `--json` (raw protocol payloads), `-q/--quiet` (tab-separated, no headers). Exit codes follow the spec: 0 ok, 1 generic, 2 usage, 3 backend unreachable, 4 not found, 5 conflict.

Architecture: Cargo workspace with three members - `src-tauri/` (the app, in place), `crates/portsage-client/` (sync UnixStream client and wire types, single source of truth), `crates/portsage-cli/` (clap-based binary). The socket protocol grew from 5 methods to 14 to match the GUI surface (see `CHANGELOG.md` [Unreleased]). The MCP server picked up the same 9 new tools so Claude can drive everything the CLI can.

Optional, no fixed timeline:
- Shell completions for bash / zsh / fish via `clap_complete`. A `portsage completion <shell>` subcommand emits the file; the Homebrew cask installs them in the right path per shell. Nice ergonomic win for power users; not urgent.

---

## 4. Project tags and colors

**Priority**: low. Nice to have, cosmetic UX improvement.

### What it does
Lets you associate each project with a color (and optionally an emoji/icon) to recognize it at a glance in the sidebar and the popover.

### UX
- "New project" and "Edit project" modals: color picker (predefined palette of 8-12 colors consistent with the dark theme)
- Sidebar: colored dot next to the project name
- Menubar popover: same, in mini version
- Active-ports badge inherits the project color
- Optional: emoji picker for the project icon

### DB schema
```sql
ALTER TABLE projects ADD COLUMN color TEXT;
ALTER TABLE projects ADD COLUMN icon TEXT;  -- emoji or lucide icon name
```

### Predefined palette
Colors that work on the Portsage dark theme with good contrast:
- amber (default), red, orange, yellow, green, cyan, blue, purple, pink, grey

### Implementation
- SQLite migration to add the columns
- Update the `create_project`, `update_project` Tauri commands
- UI: reusable ColorPicker component

---

## 5. System notifications

**Priority**: low. A differentiator vs competitors (none have native notifications).

### What it does
Sends native macOS notifications on relevant events, configurable in settings.

### Notifiable events
- **Port collision**: a registered port is taken by an unexpected process
- **Zombie port**: a registered port is active but the process does not match the configured service
- **Range exhausted**: the global base_port + range_size has few free ranges left
- **MCP reserve**: Claude reserved a new range (optional, can be noisy)
- **Process killed**: feedback on a kill via CLI/MCP for asynchronous confirmation
- **Auto-detected project**: if in the future Portsage automatically detects new docker-compose files in the filesystem

### UX
- Settings > "Notifications": toggle for each event type
- Defaults: collision and zombie port on, the rest off
- Click on the notification opens the panel of the affected project

### Implementation
- `tauri-plugin-notification` for native notifications
- Listener in the port scanner polling: compares current vs previous state, detects events
- Storage of recent events (last 50) in a SQLite table for a future timeline

### macOS permissions
- On first launch, request notification permissions
- Graceful fallback if the user denies

---

## 6. i18n and language switcher

**Priority**: medium. The app currently ships with English strings hardcoded. A proper i18n setup unlocks Italian (and any future language) without code changes.

### What it does
Replaces hardcoded UI strings with translation keys, adds a language switcher in settings, and persists the chosen language in the DB. Italian is the first additional locale (the project owner is Italian-speaking).

### UX
- Settings > "Language": dropdown with available languages (English, Italian)
- On change, the entire UI re-renders in the new language without restart
- On first launch, default language is detected from the macOS system locale (`sys-locale` Rust crate); falls back to English if unsupported
- Persisted across restarts

### Library and stack
- `react-i18next` + `i18next` - de facto standard, supports lazy loading and pluralization
- Locale files in `src/i18n/locales/{en,it}.json` with semantic keys (e.g. `settings.title`, `project.add.cta`), never positional keys
- Language stored in the SQLite `config` table (`key = "language"`)
- Loaded by the Rust backend at startup and exposed to the frontend via a Tauri command (so React can boot already in the right language without flicker)

### Implementation
1. Install `i18next` and `react-i18next`
2. Create `src/i18n/index.ts` with init logic
3. Audit all `.tsx` files for hardcoded strings and extract them into `en.json`
4. Create `it.json` with the original Italian translations preserved from before the English migration
5. Replace strings in components with the `t()` hook
6. Add the language dropdown to `SettingsPanel`
7. Wire `i18n.changeLanguage(lang)` plus DB persistence on change
8. On first launch, detect system locale via a Rust command and pass it to the frontend as the default

### Tricky bits
- **Pluralization**: "1 active port" vs "5 active ports". Use `i18next` plural rules with the `count` parameter
- **Numbers and dates**: use `Intl.NumberFormat` and `Intl.DateTimeFormat` with the current locale
- **Backend errors**: the Rust backend should return error codes, not strings; the frontend translates them
- **MCP server output**: stays in English (it is read by Claude, not by the user)
- **SKILL.md**: stays in English (it is a prompt for Claude)
- **Tooltips and aria-labels**: must be included in the audit, easy to forget
- **Title bar and tray menu items**: handled in Rust; expose a Tauri command so the Rust side can fetch translated labels from the frontend or load the JSON directly

### Languages to ship
- English (default)
- Italian (priority, the owner uses Italian daily)
- Future: any language can be added by dropping a new JSON file in `locales/`

---

## Suggested implementation order

1. ~~**Kill process** - unblocks the basic workflow, parity with competitors~~ (done)
2. ~~**Open in browser** - low effort, high value~~ (done)
3. ~~**CLI** - opens new use cases (scripting, CI)~~ (done)
4. **i18n and language switcher** - reaches Italian-speaking users and any future locale
5. **Notifications** - added value, differentiator
6. **Tags and colors** - polish, last step
