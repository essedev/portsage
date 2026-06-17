# Portsage

> 🇮🇹 [Leggi in italiano](README.it.md)

A macOS menubar app that manages port allocation across development projects, with a Linux headless server variant for remote dev boxes.

## The problem

Working with AI on 4-5 projects in parallel (React/Vite + Docker with PostgreSQL, Redis, S3) constantly causes port collisions. There is no simple way to see which ports are taken, by which project, and which ranges are still free.

## The solution

- **Menubar popover**: compact quick view to check the state of your ports.
- **Full app window**: full management of projects, ports, and settings.
- **MCP server**: integration with any MCP-compatible editor (Claude Code, Cursor, Claude Desktop, Cline, VS Code Copilot, Codex, Windsurf) to reserve ports and register services automatically.
- **CLI**: a `portsage` command in PATH for scripting, CI, and quick terminal use - bundled with the app.
- **Unmanaged ports**: detects active ports that are not associated with any project.
- **Multi-host**: configure remote Linux backends and reach them via SSH from the Mac UI, with automatic port forwarding.

## Installation

### Homebrew (macOS)

```bash
brew tap essedev/portsage
brew install portsage
```

### Linux headless server

Grab the tarball from the [latest release](https://github.com/essedev/portsage/releases/latest) and run the installer:

```bash
tar -xzf portsage-server-linux-x86_64.tar.gz
sudo ./install.sh   # installs the binary + systemd unit, rewrites User=/Group= to $SUDO_USER
```

Then point the Mac app at it via Settings > Remote backends.

### From source (development)

```bash
pnpm install         # frontend deps
pnpm tauri dev       # dev mode (hot reload)
```

The MCP server's Python deps (`mcp/pyproject.toml`) are bundled into the CLI binary and `uv sync` runs the first time you call `portsage mcp install`. You only need to `uv sync` manually in `mcp/` if you're iterating on `mcp/server.py` itself.

### Tests

```bash
cargo test    # Rust workspace (app + portsage-client + portsage-cli)
pnpm test     # TypeScript frontend (vitest)
```

## MCP integration

The MCP server exposes 15 tools across three groups:

- **Read**: `list_all`, `scan_active`, `list_unmanaged`, `next_range`, `get_config`, `find_project_by_path`.
- **Mutate**: `reserve_range`, `update_project`, `register_port`, `remove_port`, `release_project`, `set_config`.
- **Act**: `kill_port`, `kill_project`, `open_in_browser`.

**Claude Code**: install from terminal (canonical):

```bash
portsage mcp install              # patches ~/.claude.json, ~/.claude/skills/, ~/.claude/settings.json atomically
portsage mcp status               # check what's installed
portsage mcp uninstall            # remove the integration
```

Or from the app: Settings > "Configure MCP" > Claude Code.

**Other editors** (Cursor, Claude Desktop, Cline, VS Code Copilot, Codex, Windsurf): the app generates ready-to-paste config with a copy button. Settings > "Configure MCP" > "Other editors", pick your editor, paste the snippet into the editor's config file.

## CLI

Once the app is installed (Homebrew or DMG), `portsage` is available on PATH. It talks to the same Unix socket as the MCP server and auto-spawns the backend in headless mode if no instance is running.

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

portsage mcp install|uninstall|status      # manage the Claude Code MCP integration
portsage self-update                       # check for and install a newer release
```

Global flags: `--json` for machine-readable output, `-q/--quiet` for pipe-friendly tab-separated lines, `--no-autospawn` to disable backend auto-launch, `--app PATH` / `--socket PATH` to override the discovered paths, `--backend <name>` / `PORTSAGE_BACKEND` env to target a remote backend.

Exit codes: `0` success, `1` generic error, `2` usage, `3` backend unreachable, `4` not found, `5` conflict.

## Documentation

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) - architecture, modules, socket protocol, UI.
- [docs/DATABASE_SCHEMA.md](docs/DATABASE_SCHEMA.md) - SQLite tables and invariants.
- [docs/DESIGN.md](docs/DESIGN.md) - design tokens and component library.
- [docs/ROADMAP.md](docs/ROADMAP.md) - what's shipped and what's next.
- [docs/feature-proposals.md](docs/feature-proposals.md) - design sketches for upcoming features.
- [docs/RELEASING.md](docs/RELEASING.md) - how to cut a release.
- [docs/multi-host-evolution.md](docs/multi-host-evolution.md) - the multi-host plan (Phases 1-3 shipped).
- [CHANGELOG.md](CHANGELOG.md) - per-version changelog.
- [CLAUDE.md](CLAUDE.md) - project conventions for AI agents working on the codebase.

🇮🇹 Italian: [README.it.md](README.it.md), [docs/ARCHITECTURE.it.md](docs/ARCHITECTURE.it.md).

## License

[MIT](LICENSE) (c) 2026 Simone Salerno

## Links

- GitHub: <https://github.com/essedev/portsage>
- Issues: <https://github.com/essedev/portsage/issues>
