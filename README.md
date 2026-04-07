# Portsage

> 🇮🇹 [Leggi in italiano](README.it.md)

A macOS menubar app that manages port allocation across development projects.

## The problem

Working with AI on 4-5 projects in parallel (React/Vite + Docker with PostgreSQL, Redis, S3) constantly causes port collisions. There is no simple way to see which ports are taken, by which project, and which ranges are still free.

## The solution

- **Menubar popover**: compact quick view to check the state of your ports
- **Full app window**: full management of projects, ports, and settings
- **MCP server**: integration with any MCP-compatible editor (Claude Code, Cursor, Claude Desktop, Cline, VS Code Copilot, Codex, Windsurf) to reserve ports and register services automatically
- **Unmanaged ports**: detects active ports that are not associated with any project

## Installation

### Homebrew

```bash
brew tap essedev/portsage
brew install portsage
```

### From source

```bash
pnpm install
cd mcp && uv sync && cd ..
pnpm tauri build
```

## Development

```bash
pnpm install                  # frontend dependencies
cd mcp && uv sync && cd ..    # MCP dependencies
pnpm tauri dev                # dev mode (hot reload)
```

## MCP integration

The MCP server exposes 5 tools:
- `list_all` - full registry plus port status
- `reserve_range(project_name)` - reserves the next free range
- `register_port(project_name, service, port)` - registers a port
- `release_project(project_name)` - releases a range
- `scan_active` - active ports on the machine

**Claude Code**: automatic install from the app (Settings > "Configure MCP" > Claude Code) or from terminal (`mcp/install.sh`).

**Other editors** (Cursor, Claude Desktop, Cline, VS Code Copilot, Codex, Windsurf): the app generates the config ready for the chosen editor with a copy button. Go to Settings > "Configure MCP" > "Other editors", pick your editor, and paste the config into the indicated file.

## Roadmap

Next planned features (see `FEATURES_TODO.md` for details):

- Kill the process directly from the UI
- Open in browser / copy URL for HTTP ports
- CLI for scripting (`portsage reserve`, `portsage list`, etc.)
- Customizable tags and colors per project
- System notifications (collisions, zombie ports, MCP events)
- i18n and language switcher (English + Italian, more languages later)

## Documentation

- 🇬🇧 English: this README, [PROJECT.md](PROJECT.md), [DESIGN.md](DESIGN.md), [ROADMAP.md](ROADMAP.md), [FEATURES_TODO.md](FEATURES_TODO.md), [RELEASING.md](RELEASING.md), [CHANGELOG.md](CHANGELOG.md)
- 🇮🇹 Italian: [README.it.md](README.it.md), [PROJECT.it.md](PROJECT.it.md)

## License

[MIT](LICENSE) © 2026 Simone Salerno

## Links

- GitHub: https://github.com/essedev/portsage
- Documentation: see `PROJECT.md`, `DESIGN.md`, `ROADMAP.md`, `FEATURES_TODO.md`, `CHANGELOG.md`
