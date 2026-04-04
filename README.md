# Grimport

Menubar app per macOS che gestisce l'allocazione delle porte tra progetti di sviluppo.

## Problema

Lavorare con AI su 4-5 progetti in parallelo (React/Vite + Docker con PostgreSQL, Redis, Minio) causa collisioni di porte continue. Non esiste un modo semplice per vedere quali porte sono occupate, da quale progetto, e quali range sono liberi.

## Soluzione

- **Popover dalla menubar**: quick view compatta per controllare lo stato delle porte
- **Finestra app full**: gestione completa di progetti, porte, settings
- **MCP server**: Claude Code riserva porte e registra servizi automaticamente
- **Porte non gestite**: rileva porte attive non associate a nessun progetto

## Installazione

### Homebrew

```bash
brew tap essedev/grimport
brew install grimport
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

## Integrazione con Claude Code

Il MCP server espone 5 tool:
- `list_all` - registry completo + stato porte
- `reserve_range(project_name)` - riserva prossimo range libero
- `register_port(project_name, service, port)` - registra porta
- `release_project(project_name)` - libera range
- `scan_active` - porte attive sulla macchina

Installazione dall'app (Impostazioni > "Connetti a Claude Code") o da terminale (`mcp/install.sh`).

## Link

- GitHub: https://github.com/essedev/grimport
- Documentazione: vedi `PROJECT.md`, `DESIGN.md`, `ROADMAP.md`
