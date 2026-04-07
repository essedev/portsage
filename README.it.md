# Portsage

> 🇬🇧 [Read in English](README.md)

Menubar app per macOS che gestisce l'allocazione delle porte tra progetti di sviluppo.

## Problema

Lavorare con AI su 4-5 progetti in parallelo (React/Vite + Docker con PostgreSQL, Redis, Minio) causa collisioni di porte continue. Non esiste un modo semplice per vedere quali porte sono occupate, da quale progetto, e quali range sono liberi.

## Soluzione

- **Popover dalla menubar**: quick view compatta per controllare lo stato delle porte
- **Finestra app full**: gestione completa di progetti, porte, settings
- **MCP server**: integrazione con qualsiasi editor MCP-compatibile (Claude Code, Cursor, Claude Desktop, Cline, VS Code Copilot, Codex, Windsurf) per riservare porte e registrare servizi automaticamente
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

Il MCP server espone 5 tool:
- `list_all` - registry completo + stato porte
- `reserve_range(project_name)` - riserva prossimo range libero
- `register_port(project_name, service, port)` - registra porta
- `release_project(project_name)` - libera range
- `scan_active` - porte attive sulla macchina

**Claude Code**: installazione automatica dall'app (Impostazioni > "Configura MCP" > Claude Code) o da terminale (`mcp/install.sh`).

**Altri editor** (Cursor, Claude Desktop, Cline, VS Code Copilot, Codex, Windsurf): l'app genera la config gia' pronta per l'editor scelto, con bottone di copia. Vai in Impostazioni > "Configura MCP" > "Altri editor", seleziona l'editor e incolla la config nel file indicato.

## Roadmap

Prossime feature pianificate (vedi `FEATURES_TODO.md` per dettagli):

- Kill del processo direttamente dalla UI
- Open in browser / copy URL per porte HTTP
- CLI per scripting (`portsage reserve`, `portsage list`, ecc.)
- Tag e colori personalizzabili per progetti
- Notifiche di sistema (collisioni, porte zombie, eventi MCP)
- i18n e cambio lingua nell'app (italiano + inglese, altre in futuro)

## Licenza

[MIT](LICENSE) © 2026 Simone Salerno

## Link

- GitHub: https://github.com/essedev/portsage
- Documentazione: vedi `PROJECT.md`, `DESIGN.md`, `ROADMAP.md`, `FEATURES_TODO.md`, `CHANGELOG.md`
