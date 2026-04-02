# Grimport

Menubar app per macOS che gestisce l'allocazione delle porte tra progetti di sviluppo.

## Problema

Lavorare con AI su 4-5 progetti in parallelo (React/Vite + Docker con PostgreSQL, Redis, Minio) causa collisioni di porte continue. Non esiste un modo semplice per vedere quali porte sono occupate, da quale progetto, e quali range sono liberi.

## Soluzione

Un'app macOS con:
- **Popover dalla menubar**: quick view compatta per controllare lo stato delle porte
- **Finestra app full**: gestione completa di progetti, porte, settings
- **MCP server**: Claude Code riserva porte e registra servizi automaticamente

## Architettura

### Componenti

1. **App Tauri** (Tauri v2 + React + Tailwind)
   - Rust backend: unico owner dello stato, gestisce DB e port scanning
   - Espone un Unix socket locale (`~/.config/grimport/grimport.sock`) per il MCP server
   - Frontend React per popover e finestra full

2. **MCP server** (Python, thin client)
   - Riceve chiamate da Claude Code
   - Le inoltra al Tauri backend via Unix socket
   - Non accede al DB direttamente

3. **Database** (SQLite)
   - Path: `~/.config/grimport/grimport.db`
   - Source of truth, gestito esclusivamente dal Rust backend

### Flusso dati

```
Claude Code  -->  MCP server (Python)  -->  Unix socket  -->  Tauri (Rust)  -->  SQLite
                                                                  |
UI (React)  <--  Tauri IPC commands  <-----------------------------
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

L'app scanna le porte attive con `lsof -iTCP -sTCP:LISTEN` e incrocia con il DB per mostrare lo stato in tempo reale.

## Stack

- **Frontend**: React 19 + TypeScript + Tailwind v4
- **App shell**: Tauri v2 (Rust) - menubar, system tray, popover, finestra app
- **MCP server**: Python + mcp SDK (thin client)
- **Package manager**: pnpm (frontend), uv (Python)
- **Database**: SQLite in `~/.config/grimport/`

## Distribuzione

- **Dev**: `pnpm tauri dev`
- **Produzione**: Homebrew tap (`.dmg` buildata con `tauri build`)

## Integrazione con Claude Code

Nel CLAUDE.md globale (`~/.claude/CLAUDE.md`):

```
## Port management
Prima di assegnare porte a un nuovo progetto, usa il MCP grimport per ottenere un range libero.
Registra sempre le porte usate tramite il MCP.
```

Claude chiama `reserve_range("nuovo-progetto")`, riceve `[4020, 4029]`, usa quelle porte nel docker-compose e vite.config.

## UI

### Popover menubar (quick view)

Pannello compatto (~350px), solo lettura, per check veloce.

```
+----------------------------------+
|  Grimport              7 active  |
+----------------------------------+
|  progetto-a        4000-4009     |
|    vite       4000  [green]      |
|    postgres   4001  [green]      |
|    redis      4002  [red]        |
|                                  |
|  progetto-b        4010-4019     |
|    vite       4010  [green]      |
|    postgres   4011  [green]      |
|    redis      4012  [green]      |
|    minio      4013  [red]        |
+----------------------------------+
|  Apri Grimport                   |
+----------------------------------+
```

- Lista progetti con porte e stato attivo (verde/rosso)
- Click su progetto -> apre finestra full su quel progetto
- Link "Apri Grimport" per la finestra completa

### Finestra app full

Finestra ridimensionabile per gestione completa.

**Sidebar**
- Lista progetti con indicatore stato (porte attive/totali)
- Campo ricerca/filtro
- Bottone "+" per nuovo progetto

**Dettaglio progetto** (pannello principale)
- Nome, path, range assegnato
- Lista servizi: nome, porta, stato attivo (verde/rosso)
- Aggiunta/rimozione servizi e porte
- Azioni: rimuovi progetto, copia porte, apri path nel terminale

**Settings**
- Configurazione base_port e range_size
- Export: esporta DB + preferenze in un file `.grimport` (zip con SQLite dump + config JSON)
- Import: importa file `.grimport` per ripristinare su un'altra macchina o da backup
- Notifiche collisioni (on/off)

### Icona menubar

- Icona statica nella status bar
- Badge numerico con progetti che hanno porte attive
