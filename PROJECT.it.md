# Portsage

> 🇬🇧 [Read in English](PROJECT.md)

Menubar app per macOS che gestisce l'allocazione delle porte tra progetti di sviluppo.

## Problema

Lavorare con AI su 4-5 progetti in parallelo (React/Vite + Docker con PostgreSQL, Redis, Minio) causa collisioni di porte continue. Non esiste un modo semplice per vedere quali porte sono occupate, da quale progetto, e quali range sono liberi.

## Soluzione

Un'app macOS con:
- **Popover dalla menubar**: quick view compatta per controllare lo stato delle porte
- **Finestra app full**: gestione completa di progetti, porte, settings
- **MCP server**: Claude Code riserva porte e registra servizi automaticamente
- **Porte non gestite**: rileva porte attive non associate a nessun progetto

## Architettura

### Componenti

1. **App Tauri** (Tauri v2 + React + Tailwind)
   - Rust backend: unico owner dello stato, gestisce DB e port scanning
   - Espone un Unix socket locale (`~/.config/portsage/portsage.sock`) per il MCP server
   - Frontend React per popover e finestra full

2. **MCP server** (Python, thin client)
   - Riceve chiamate da Claude Code via stdio
   - Le inoltra al Tauri backend via Unix socket
   - Non accede al DB direttamente

3. **Database** (SQLite)
   - Path: `~/.config/portsage/portsage.db`
   - Source of truth, gestito esclusivamente dal Rust backend

### Flusso dati

```
Claude Code  -->  MCP server (Python/stdio)  -->  Unix socket  -->  Tauri (Rust)  -->  SQLite
                                                                         |
UI (React)  <--  Tauri IPC commands  <------------------------------------
```

### Database schema

```sql
CREATE TABLE projects (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    path TEXT,
    range_start INTEGER NOT NULL,
    range_end INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE ports (
    id INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL REFERENCES projects(id),
    service TEXT NOT NULL,
    port INTEGER NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
-- Default: base_port = 4000, range_size = 10
```

### Port scanning

L'app scanna le porte attive con `lsof -iTCP -sTCP:LISTEN -nP` e incrocia con il DB per mostrare lo stato in tempo reale. Il nome processo viene risolto via `ps -p <pid> -o comm=` per evitare il troncamento di lsof.

Le porte non gestite sono filtrate: solo porte >= 3000, esclusi processi di sistema (AirPlay, CUPS, mDNS, Spotlight, etc.).

## Stack

- **Frontend**: React 19 + TypeScript + Tailwind v4
- **App shell**: Tauri v2 (Rust) - menubar, system tray, popover, finestra app
- **MCP server**: Python + FastMCP SDK (thin client via stdio)
- **Package manager**: pnpm (frontend), uv (Python)
- **Database**: SQLite in `~/.config/portsage/`
- **Font**: system-ui (UI), ui-monospace (titoli e dati tecnici)

## Distribuzione

- **Dev**: `pnpm tauri dev`
- **Build**: `pnpm tauri build` (genera `.dmg`)
- **Homebrew**: `brew tap essedev/portsage && brew install portsage`
- **GitHub**: https://github.com/essedev/portsage

## Integrazione MCP

Il MCP server espone 5 tool:
- `list_all` - registry completo + stato porte
- `reserve_range(project_name)` - riserva prossimo range libero
- `register_port(project_name, service, port)` - registra porta
- `release_project(project_name)` - libera range
- `scan_active` - porte attive sulla macchina

**Claude Code**: installazione automatica dall'app (Impostazioni > "Configura MCP" > Claude Code) o da terminale (`mcp/install.sh`). Installa MCP server, skill file e permessi tool.

**Altri editor**: l'app supporta config copy-paste per Cursor, Claude Desktop, Cline, VS Code (Copilot), Codex (TOML), Windsurf. La sezione "Altri editor" in Impostazioni > "Configura MCP" mostra dropdown editor + config gia' generata col path corretto della directory MCP, pronta da incollare nel file di config dell'editor.

I file MCP server (`server.py`, `pyproject.toml`, `SKILL.md`) vengono bundlati come risorse nell'app `.dmg` e copiati in `~/Library/Application Support/portsage/mcp/` al primo uso (vedi `commands::get_mcp_dir`).

## UI

### Popover menubar (quick view)

Pannello compatto (350x480px), solo lettura, per check veloce.

- Header con titolo "portsage" e badge porte attive
- Lista progetti con porte e stato attivo (pallino ambra/grigio)
- Footer con conteggio porte e link "Apri portsage"

### Finestra app full

Finestra ridimensionabile (min 700x400) con titlebar trasparente.

**Header**
- Titolo "portsage" con glow ambra + tagline "ports under control"
- Badge porte attive

**Sidebar**
- Campo ricerca/filtro
- Bottone "Nuovo progetto"
- Lista progetti con badge porte attive
- Sezione "Non gestite" con conteggio (visibile solo se presenti)
- Bottone "Configura MCP" (visibile solo se MCP non installato per Claude Code)
- Bottone "Impostazioni"

**Dettaglio progetto** (pannello principale)
- Nome, path, range assegnato, badge porte attive
- Lista servizi: nome servizio, porta, stato attivo, nome processo
- Aggiunta servizi con dropdown porte libere nel range
- Azioni: rimuovi progetto, apri nel Finder, apri nel Terminale

**Porte non gestite**
- Tabella con porta, nome processo, PID
- Solo porte >= 3000, filtrate da processi di sistema

**Impostazioni**
- Avvio automatico al login (toggle)
- Configurazione base_port e range_size
- Configura MCP: due sezioni
  - **Claude Code**: stato connessione, installa/rimuovi, lista tool disponibili
  - **Altri editor**: dropdown editor + config copy-paste con istruzioni per Cursor, Windsurf, VS Code, Claude Desktop, Continue, Cline, Codex, Zed
- Export/import dati (.portsage = zip con DB + config)

### Icona menubar

- Icona template (adatta a dark/light mode di macOS)
