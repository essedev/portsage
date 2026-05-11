# Portsage

> 🇮🇹 [Leggi in italiano](README.it.md)

A macOS menubar app that manages port allocation across development projects.

## The problem

Working with AI on 4-5 projects in parallel (React/Vite + Docker with PostgreSQL, Redis, S3) constantly causes port collisions. There is no simple way to see which ports are taken, by which project, and which ranges are still free.

## The solution

- **Menubar popover**: compact quick view to check the state of your ports
- **Full app window**: full management of projects, ports, and settings
- **MCP server**: integration with any MCP-compatible editor (Claude Code, Cursor, Claude Desktop, Cline, VS Code Copilot, Codex, Windsurf) to reserve ports and register services automatically
- **CLI**: a `portsage` command in PATH for scripting, CI, and quick terminal use - bundled with the app
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

### Tests

```bash
cd src-tauri && cargo test    # Rust backend (db, scanner, socket, commands)
pnpm test                     # TypeScript frontend (vitest)
```

## MCP integration

The MCP server exposes 14 tools across three groups:

- Read: `list_all`, `scan_active`, `list_unmanaged`, `next_range`, `get_config`, `find_project_by_path`
- Mutate: `reserve_range`, `register_port`, `remove_port`, `release_project`, `set_config`
- Act: `kill_port`, `kill_project`, `open_in_browser`

**Claude Code**: automatic install from the app (Settings > "Configure MCP" > Claude Code) or from terminal (`mcp/install.sh`).

**Other editors** (Cursor, Claude Desktop, Cline, VS Code Copilot, Codex, Windsurf): the app generates the config ready for the chosen editor with a copy button. Go to Settings > "Configure MCP" > "Other editors", pick your editor, and paste the config into the indicated file.

## CLI

Once the app is installed (Homebrew or DMG), `portsage` is available in PATH. It talks to the same Unix socket as the MCP server, and auto-spawns the backend in headless mode if no instance is running.

```bash
portsage list                              # all projects and their ports
portsage list --here                       # the project the cwd belongs to
portsage list --active                     # only active ports

portsage status                            # short detail for the cwd project
portsage reserve myapp --here              # reserve a range, bind it to the cwd
portsage register vite 4000 --here         # register a service in the cwd project
portsage remove vite --here                # remove a service
portsage release --here                    # delete the cwd project's range (asks to confirm; -y to skip)

portsage scan                              # active ports on the machine
portsage scan --unmanaged                  # only ports not owned by any project
portsage kill 4000                         # SIGTERM with 2s grace, then SIGKILL (asks to confirm; -y)
portsage kill-project --here               # kill every active port in the cwd project, in parallel
portsage open 4000                         # open http://localhost:4000 in the default browser

portsage config get                        # read base_port / range_size
portsage config set range_size=20

portsage doctor                            # diagnose the local install (socket reachable, app found, etc.)
```

Global flags: `--json` for machine-readable output, `-q/--quiet` for pipe-friendly tab-separated lines, `--no-autospawn` to disable backend auto-launch, `--app PATH` and `--socket PATH` to override the discovered paths.

Exit codes: `0` success, `1` generic error, `2` usage, `3` backend unreachable, `4` not found, `5` conflict.

## Roadmap

Next planned features (see `FEATURES_TODO.md` for details):

- Copy URL for HTTP ports
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
