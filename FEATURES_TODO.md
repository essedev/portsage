# Features TODO

Features planned to reach parity with existing competitors (Portkeeper, Port Manager, Port Killer) and beyond. Each one is self-contained and can be implemented independently.

---

## 1. Kill process from the UI

**Priority**: high. It is the basic workflow in existing port managers. Its absence is the first thing users notice.

### What it does
Lets you terminate a process occupying a port directly from the UI, without dropping into the terminal.

### UX
- In the project detail service list: a "kill" button next to the active port
- In the "Unmanaged ports" table: a "kill" button next to the port
- Confirmation before killing (modal or "click again to confirm" tooltip)
- Visual feedback: port goes from active (amber) to inactive (grey) after kill

### Implementation
- Tauri command `kill_port(port: u16)` in the Rust backend
- Runs `kill -TERM <pid>` (graceful) and falls back to `kill -9 <pid>`
- The PID is already available from the port scanner (`lsof -iTCP -sTCP:LISTEN -nP`)
- Automatic refresh of the port scanner after kill

### MCP
- Add a `kill_port(port: int)` tool to the MCP server
- Claude can kill a zombie process on request

### Edge cases
- System (root) process: clear error, suggest sudo
- Already-dead process: silent refresh
- Port not found: explicit error

---

## 2. Open in browser / Copy URL

**Priority**: medium. Low effort, high perceived value.

### What it does
For HTTP ports (Vite, FastAPI, etc.), a click opens `http://localhost:PORT` in the default browser. Alternatively, copies the URL to the clipboard.

### UX
- Click on the port number: opens in the browser
- Cmd+click or right-click: copies the URL to the clipboard with a "Copied" toast
- Discreet icon next to HTTP-friendly ports indicating the available action

### Which ports are "HTTP"?
- Simple heuristic: any port registered with a known service (vite, fastapi, next, react, etc.)
- Service name in the registry: if it contains "vite|frontend|api|backend|web", treat as HTTP
- Settings: editable whitelist/blacklist of service names
- Fallback: an "open as HTTP" button always available

### Implementation
- Tauri command `open_in_browser(url: String)` using the `open` crate or `tauri-plugin-shell`
- Command `copy_to_clipboard(text: String)` using `tauri-plugin-clipboard-manager`
- UI: hover on port numbers shows available actions

---

## 3. CLI

**Priority**: medium. Unlocks scripting/CI use cases the GUI does not cover.

### What it does
A `portsage` terminal command that talks to the Tauri backend via the same Unix socket the MCP server uses.

### Planned commands
```bash
portsage list                              # all projects and ports
portsage list --active                     # only active ports
portsage list --project myapp              # ports of a specific project
portsage reserve myapp                     # reserve a range for a new project
portsage reserve myapp --size 20           # custom range
portsage register myapp vite 4000          # register a service
portsage release myapp                     # release a range
portsage scan                              # active ports on the machine
portsage scan --unmanaged                  # only unmanaged ports
portsage kill 4000                         # kill the process on a port
portsage open 4000                         # open in browser
portsage --json                            # JSON output for scripting
```

### Implementation
- A separate `portsage-cli` binary in Rust within the monorepo
- Reuses the same protocol as the MCP server (Unix socket -> Tauri backend)
- Distribution: bundled inside the `.dmg`, symlinked to `/usr/local/bin/portsage` on first launch
- Homebrew: the formula installs the CLI binary too

### Output
- Default: colored ASCII table
- `--json`: for scripting
- `--quiet`: only essential data

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

1. **Kill process** - unblocks the basic workflow, parity with competitors
2. **Open in browser** - low effort, high value
3. **CLI** - opens new use cases (scripting, CI)
4. **i18n and language switcher** - reaches Italian-speaking users and any future locale
5. **Notifications** - added value, differentiator
6. **Tags and colors** - polish, last step
