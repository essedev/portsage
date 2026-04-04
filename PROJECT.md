# grimport

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
   - Espone un Unix socket locale (`~/.config/grimport/grimport.sock`) per il MCP server
   - Frontend React per popover e finestra full

2. **MCP server** (Python, thin client)
   - Riceve chiamate da Claude Code via stdio
   - Le inoltra al Tauri backend via Unix socket
   - Non accede al DB direttamente

3. **Database** (SQLite)
   - Path: `~/.config/grimport/grimport.db`
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
- **Database**: SQLite in `~/.config/grimport/`
- **Font**: system-ui (UI), ui-monospace (titoli e dati tecnici)

## Distribuzione

- **Dev**: `pnpm tauri dev`
- **Build**: `pnpm tauri build` (genera `.dmg`)
- **Homebrew**: `brew tap essedev/grimport && brew install grimport`
- **GitHub**: https://github.com/essedev/grimport

## Integrazione con Claude Code

Il MCP server espone 5 tool:
- `list_all` - registry completo + stato porte
- `reserve_range(project_name)` - riserva prossimo range libero
- `register_port(project_name, service, port)` - registra porta
- `release_project(project_name)` - libera range
- `scan_active` - porte attive sulla macchina

L'installazione avviene dall'app (Impostazioni > "Connetti a Claude Code") o da terminale (`mcp/install.sh`). Installa MCP server, skill file e permessi tool.

## UI

### Popover menubar (quick view)

Pannello compatto (350x480px), solo lettura, per check veloce.

- Header con titolo "grimport" e badge porte attive
- Lista progetti con porte e stato attivo (pallino ambra/grigio)
- Footer con conteggio porte e link "Apri grimport"

### Finestra app full

Finestra ridimensionabile (min 700x400) con titlebar trasparente.

**Header**
- Titolo "grimport" con glow ambra + tagline "your port grimoire"
- Badge porte attive

**Sidebar**
- Campo ricerca/filtro
- Bottone "Nuovo progetto"
- Lista progetti con badge porte attive
- Sezione "Non gestite" con conteggio (visibile solo se presenti)
- Bottone "Connetti a Claude Code" (visibile solo se MCP non installato)
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
- Integrazione Claude Code: stato connessione, installa/rimuovi, lista tool disponibili
- Export/import dati (.grimport = zip con DB + config)

### Icona menubar

- Icona template (adatta a dark/light mode di macOS)
