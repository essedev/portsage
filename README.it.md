# Portsage

> 🇬🇧 [Read in English](README.md)

Menubar app per macOS che gestisce l'allocazione delle porte tra progetti di sviluppo.

## Problema

Lavorare con AI su 4-5 progetti in parallelo (React/Vite + Docker con PostgreSQL, Redis, Minio) causa collisioni di porte continue. Non esiste un modo semplice per vedere quali porte sono occupate, da quale progetto, e quali range sono liberi.

## Soluzione

- **Popover dalla menubar**: quick view compatta per controllare lo stato delle porte
- **Finestra app full**: gestione completa di progetti, porte, settings
- **MCP server**: integrazione con qualsiasi editor MCP-compatibile (Claude Code, Cursor, Claude Desktop, Cline, VS Code Copilot, Codex, Windsurf) per riservare porte e registrare servizi automaticamente
- **CLI**: un comando `portsage` in PATH per scripting, CI e uso veloce da terminale - distribuito insieme all'app
- **Porte non gestite**: rileva porte attive non associate a nessun progetto

## Installazione

### Homebrew

```bash
brew tap essedev/portsage
brew install portsage
```

### Da sorgente

```bash
pnpm install
cd mcp && uv sync && cd ..
pnpm tauri build
```

## Sviluppo

```bash
pnpm install                  # dipendenze frontend
cd mcp && uv sync && cd ..    # dipendenze MCP
pnpm tauri dev                # dev mode (hot reload)
```

## Integrazione MCP

Il MCP server espone 14 tool in tre gruppi:

- Lettura: `list_all`, `scan_active`, `list_unmanaged`, `next_range`, `get_config`, `find_project_by_path`
- Mutazione: `reserve_range`, `register_port`, `remove_port`, `release_project`, `set_config`
- Azione: `kill_port`, `kill_project`, `open_in_browser`

**Claude Code**: installazione automatica dall'app (Impostazioni > "Configura MCP" > Claude Code) o da terminale (`mcp/install.sh`).

**Altri editor** (Cursor, Claude Desktop, Cline, VS Code Copilot, Codex, Windsurf): l'app genera la config già pronta per l'editor scelto, con bottone di copia. Vai in Impostazioni > "Configura MCP" > "Altri editor", seleziona l'editor e incolla la config nel file indicato.

## CLI

Una volta installata l'app (Homebrew o DMG), `portsage` è disponibile in PATH. Parla con lo stesso socket Unix usato dal server MCP e, se nessuna istanza del backend è attiva, ne avvia una in modalità headless.

```bash
portsage list                              # tutti i progetti e le loro porte
portsage list --here                       # il progetto a cui appartiene la cwd
portsage list --active                     # solo porte attive

portsage status                            # dettaglio breve per il progetto della cwd
portsage reserve myapp --here              # riserva un range e lo lega alla cwd
portsage register vite 4000 --here         # registra un servizio nel progetto della cwd
portsage remove vite --here                # rimuove un servizio
portsage release --here                    # elimina il range del progetto cwd (chiede conferma; -y per saltarla)

portsage scan                              # porte attive sulla macchina
portsage scan --unmanaged                  # solo porte non associate ad alcun progetto
portsage kill 4000                         # SIGTERM con 2s di grazia, poi SIGKILL (conferma; -y per saltarla)
portsage kill-project --here               # uccide in parallelo tutte le porte attive del progetto cwd
portsage open 4000                         # apre http://localhost:4000 nel browser di default

portsage config get                        # legge base_port / range_size
portsage config set range_size=20

portsage doctor                            # diagnostica installazione locale (socket, app, etc.)
```

Flag globali: `--json` per output machine-readable, `-q/--quiet` per output tab-separated pipe-friendly, `--no-autospawn` per disabilitare l'auto-avvio del backend, `--app PATH` e `--socket PATH` per forzare i path.

Exit code: `0` ok, `1` errore generico, `2` errore di utilizzo, `3` backend irraggiungibile, `4` non trovato, `5` conflitto.

## Roadmap

Prossime feature pianificate (vedi `FEATURES_TODO.md` per dettagli):

- Copy URL per porte HTTP
- CLI per scripting (`portsage reserve`, `portsage list`, ecc.)
- Tag e colori personalizzabili per progetti
- Notifiche di sistema (collisioni, porte zombie, eventi MCP)
- i18n e cambio lingua nell'app (italiano + inglese, altre in futuro)

## Licenza

[MIT](LICENSE) © 2026 Simone Salerno

## Link

- GitHub: https://github.com/essedev/portsage
- Documentazione: vedi `PROJECT.md`, `DESIGN.md`, `ROADMAP.md`, `FEATURES_TODO.md`, `CHANGELOG.md`
