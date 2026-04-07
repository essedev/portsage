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

- [ ] **Kill process from the UI** - kill action on active ports (basic workflow in port managers)
- [ ] **Open in browser / Copy URL** - for HTTP ports, click opens `localhost:PORT` in the default browser
- [ ] **CLI** - `portsage` command for scripting (`portsage reserve`, `portsage list`, etc.)
- [ ] **Project tags and colors** - visual customization to recognize projects at a glance
- [ ] **System notifications** - macOS alerts for collisions, zombie ports, MCP events
- [ ] **i18n and language switcher** - proper i18next setup, English + Italian, language switcher in settings, persisted in DB

## v0.7 - CI/CD and cross-platform

- [ ] GitHub Action for automatic builds on push/release
- [ ] Universal macOS binary (arm64 + x86_64)
- [ ] Linux support: adapt scanner (lsof/ss), tray icon, activation policy
- [ ] Windows support: Unix socket -> named pipe, scanner via netstat/API, OS-specific commands
- [ ] Cross-platform tests in CI
