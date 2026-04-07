# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/essedev/portsage/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/essedev/portsage/releases/tag/v0.6.0
