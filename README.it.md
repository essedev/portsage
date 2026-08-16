# Portsage

> 🇬🇧 [Read in English](README.md)

Menubar app per macOS che gestisce l'allocazione delle porte tra progetti di sviluppo, con una variante server headless per Linux per le dev box remote.

## Problema

Lavorare con AI su 4-5 progetti in parallelo (React/Vite + Docker con PostgreSQL, Redis, Minio) causa collisioni di porte continue. Non esiste un modo semplice per vedere quali porte sono occupate, da quale progetto, e quali range sono liberi.

## Soluzione

- **Popover dalla menubar**: quick view compatta per controllare lo stato delle porte.
- **Finestra app full**: gestione completa di progetti, porte, settings.
- **MCP server**: integrazione con qualsiasi editor MCP-compatibile (Claude Code, Cursor, Claude Desktop, Cline, VS Code Copilot, Codex, Windsurf) per riservare porte e registrare servizi automaticamente.
- **CLI**: un comando `portsage` in PATH per scripting, CI e uso veloce da terminale - distribuito insieme all'app.
- **Porte non gestite**: rileva porte attive non associate a nessun progetto.
- **Cestino**: progetti e porte cancellati restano recuperabili 30 giorni, col loro range originale.
- **Prune**: archivia i progetti la cui cartella non esiste più o che non tocchi da mesi, tenendo range e porte.
- **Multi-host**: configura backend Linux remoti e raggiungili via SSH dalla UI Mac, con port forwarding automatico.

## Installazione

### Homebrew (macOS)

```bash
brew tap essedev/portsage
brew install portsage
```

### Server headless Linux

Scarica il tarball dall'[ultima release](https://github.com/essedev/portsage/releases/latest) e lancia l'installer:

```bash
tar -xzf portsage-server-linux-x86_64.tar.gz
sudo ./install.sh   # installa binario + unit systemd, riscrive User=/Group= a $SUDO_USER
```

Poi punta l'app Mac al server da Impostazioni > Remote backends.

### Da sorgente (sviluppo)

```bash
pnpm install         # dipendenze frontend
pnpm tauri dev       # dev mode (hot reload)
```

Le dipendenze Python del server MCP (`mcp/pyproject.toml`) sono embedded nel binario del CLI e `uv sync` viene lanciato la prima volta che chiami `portsage mcp install`. Lo `uv sync` manuale in `mcp/` serve solo se stai iterando su `mcp/server.py` direttamente.

### Test

```bash
cargo test    # workspace Rust (app + portsage-client + portsage-cli)
pnpm test     # frontend TypeScript (vitest)
```

## Integrazione MCP

Il server MCP espone 14 tool in tre gruppi:

- **Lettura**: `list_all`, `scan_active`, `list_unmanaged`, `next_range`, `get_config`, `find_project_by_path`.
- **Mutazione**: `reserve_range`, `register_port`, `remove_port`, `release_project`, `set_config`.
- **Azione**: `kill_port`, `kill_project`, `open_in_browser`.

**Claude Code**: install da terminale (canonico):

```bash
portsage mcp install              # patcha ~/.claude.json, ~/.claude/skills/, ~/.claude/settings.json atomicamente
portsage mcp status               # verifica cosa e' installato
portsage mcp uninstall            # rimuove l'integrazione
```

Oppure dall'app: Impostazioni > "Configura MCP" > Claude Code.

**Altri editor** (Cursor, Claude Desktop, Cline, VS Code Copilot, Codex, Windsurf): l'app genera la config pronta da incollare con bottone di copia. Impostazioni > "Configura MCP" > "Altri editor", seleziona l'editor, incolla lo snippet nel file di config dell'editor.

## CLI

Una volta installata l'app (Homebrew o DMG), `portsage` e' disponibile in PATH. Parla con lo stesso socket Unix usato dal server MCP e, se nessuna istanza del backend e' attiva, ne avvia una in modalita' headless.

```bash
portsage list                              # tutti i progetti e le loro porte
portsage list --here                       # il progetto a cui appartiene la cwd
portsage list --active                     # solo porte attive

portsage status                            # dettaglio breve per il progetto della cwd
portsage reserve myapp --here              # riserva un range e lo lega alla cwd
portsage register vite 4000 --here         # registra un servizio nel progetto della cwd
portsage remove vite --here                # rimuove un servizio
portsage rename vecchio nuovo --path /nuovo/path  # rinomina un progetto e/o ne cambia il path
portsage release --here                    # elimina il range del progetto cwd (conferma; -y per saltarla)

portsage scan                              # porte attive sulla macchina
portsage scan --unmanaged                  # solo porte non associate ad alcun progetto
portsage kill 4000                         # SIGTERM con 2s di grazia, poi SIGKILL (conferma; -y)
portsage kill-project --here               # uccide in parallelo tutte le porte attive del progetto cwd
portsage open 4000                         # apre http://localhost:4000 nel browser di default

portsage prune                             # progetti con la cartella sparita o fermi da mesi (solo report)
portsage prune --days 60 --apply           # li archivia: range e porte restano riservati
portsage archive <nome>                    # archivia un progetto
portsage unarchive <nome>                  # lo rimette in lista
portsage list --archived                   # include gli archiviati nell'elenco

portsage trash list                        # progetti e porte cancellati, recuperabili 30 giorni
portsage trash restore 3                   # ripristina una voce col suo range originale
portsage trash purge 3 | --all             # cancella definitivamente (conferma; -y)

portsage config get                        # legge base_port / range_size
portsage config set range_size=20

portsage doctor                            # diagnostica installazione locale (socket, app, ecc.)

portsage mcp install|uninstall|status      # gestisce l'integrazione MCP con Claude Code
portsage self-update                       # cerca e installa la nuova release
```

Flag globali: `--json` per output machine-readable, `-q/--quiet` per output tab-separated pipe-friendly, `--no-autospawn` per disabilitare l'auto-avvio del backend, `--app PATH` / `--socket PATH` per forzare i path, `--backend <nome>` / env `PORTSAGE_BACKEND` per puntare a un backend remoto.

Exit code: `0` ok, `1` errore generico, `2` errore di utilizzo, `3` backend irraggiungibile, `4` non trovato, `5` conflitto.

## Documentazione

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) - architettura, moduli, protocollo socket, UI (inglese).
- [docs/ARCHITECTURE.it.md](docs/ARCHITECTURE.it.md) - versione italiana.
- [docs/DATABASE_SCHEMA.md](docs/DATABASE_SCHEMA.md) - schema SQLite e invarianti.
- [docs/DESIGN.md](docs/DESIGN.md) - design tokens e component library.
- [docs/ROADMAP.md](docs/ROADMAP.md) - cosa e' stato fatto e cosa rimane.
- [docs/feature-proposals.md](docs/feature-proposals.md) - design delle feature non ancora implementate.
- [docs/RELEASING.md](docs/RELEASING.md) - come fare una release.
- [docs/multi-host-evolution.md](docs/multi-host-evolution.md) - piano multi-host (Fasi 1-3 fatte).
- [CHANGELOG.md](CHANGELOG.md) - changelog per versione.
- [CLAUDE.md](CLAUDE.md) - convenzioni di progetto per agenti AI che lavorano sulla codebase.

## Licenza

[MIT](LICENSE) (c) 2026 Simone Salerno

## Link

- GitHub: <https://github.com/essedev/portsage>
- Issues: <https://github.com/essedev/portsage/issues>
