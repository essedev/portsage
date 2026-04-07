# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.2] - 2026-04-07

### Fixed
- Duplicate tray icons when more than one Portsage process was running (e.g. dev build + installed .app, or accidental relaunch): added `tauri-plugin-single-instance` so a second launch focuses the existing instance instead of spawning a new one
- Main window now opens at first launch again (regression introduced in 0.7.1 where the window was hidden at startup)

## [0.7.1] - 2026-04-07

### Fixed
- Tray icon missing at launch: the main window no longer opens visible at startup, so the app starts as a pure menubar app and the status bar icon is shown immediately
- Reopening the app from Spotlight/Finder/Dock after closing it with the X now correctly re-shows the main window via `RunEvent::Reopen`, instead of leaving a dock icon with no UI and forcing a force-quit

## [0.6.1] - 2026-04-07

### Added
- Error toast in the main window: failed actions (duplicate name, port outside range, etc.) now surface as a dismissible bottom-right banner instead of being silently logged to the console
- `humanizeError` translation layer: common SQLite/IO errors are mapped to readable messages, with raw text as fallback
- Documentation of the MCP Unix socket protocol in `PROJECT.md` (transport, methods, request/response envelopes, example session)

### Changed
- Hardened the MCP Unix socket: parent dir is now `0700` and the socket file `0600`, so only the current user can connect
- Idle socket connections are closed after 60 seconds, preventing leaked client tasks from accumulating
- Python MCP client (`mcp/server.py`) now applies a 5s timeout on connect/send/recv, so a stuck backend can no longer hang Claude Code
- Centralized the database path: `commands::export_data` and `commands::import_data` now use `Database::db_path()` instead of recomputing it
- Toast position: bottom-right (was bottom-center)

### Fixed
- Race condition in `Database::create_project`: range computation and insert are now performed under a single mutex lock, so two concurrent project creations can no longer produce overlapping port ranges
- Replaced all `.expect()` calls in critical startup paths (`lib.rs`, `socket.rs`) with graceful error handling - the app no longer panics on DB init or socket bind failures
- Replaced all `Mutex::lock().unwrap()` calls in `db.rs` with a poisoning-safe helper, eliminating the theoretical panic on a poisoned mutex
- Removed `Path::parent().unwrap()` in `commands::install_mcp`

### Planned
- Kill process from the UI
- Open in browser / copy URL for HTTP ports
- CLI for scripting
- i18n and language switcher (English + Italian)
- Project tags and colors
- System notifications

## [0.6.0] - 2026-04-07

### Added
- Initial public release as **Portsage** (renamed from internal codename `grimport`)
- macOS menubar app with popover and full window
- SQLite-backed project registry with port range allocation
- Real-time port scanning via `lsof` with process name resolution
- Unmanaged ports detection for active ports above 3000
- MCP server (Python, thin client) connecting to a local Unix socket
- One-click MCP installation for Claude Code from Settings
- Copy-paste MCP config for Cursor, Claude Desktop, Cline, VS Code Copilot, Codex (TOML), Windsurf
- Auto-launch at login (toggle in Settings)
- Configurable `base_port` and `range_size`
- Export and import data as `.portsage` archive (SQLite dump + config)
- "Open in Finder" and "Open in Terminal" shortcuts on project paths
- Search/filter projects in the sidebar
- Homebrew distribution via `essedev/portsage` tap
- English UI strings throughout the app
- English documentation (README, PROJECT, DESIGN, ROADMAP, FEATURES_TODO, RELEASING) with Italian companions for README and PROJECT
- MIT License

[Unreleased]: https://github.com/essedev/portsage/compare/v0.6.1...HEAD
[0.6.1]: https://github.com/essedev/portsage/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/essedev/portsage/releases/tag/v0.6.0
