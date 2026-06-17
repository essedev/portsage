# Portsage - Architettura

> 🇬🇧 [Read in English](ARCHITECTURE.md)

Menubar app per macOS che gestisce l'allocazione delle porte tra progetti di sviluppo, con una variante server headless per Linux e una modalita' multi-host nella UI per dev box remote.

## Problema

Lavorare con AI su 4-5 progetti in parallelo (React/Vite + Docker con PostgreSQL, Redis, Minio) causa collisioni di porte continue. Non esiste un modo semplice per vedere quali porte sono occupate, da quale progetto, e quali range sono liberi. Il problema peggiora quando i progetti vivono su una Linux box raggiunta via SSH.

## Componenti

```
+-----------------------------------------------------------------+
|  App macOS (Tauri v2 + React)                                   |
|  +---------------+    +---------------------+   +------------+  |
|  | Tray + popover|    | Finestra full (React)|  | Settings   |  |
|  +-------+-------+    +----------+----------+   +-----+------+  |
|          +-------------- IPC ----+---------------------+        |
|                              |                                  |
|  +---------------------------v------------------------------+   |
|  | Backend Rust                                             |   |
|  |   db.rs / actions.rs / commands.rs / socket.rs           |   |
|  |   scanner.rs (per-OS) / backends.rs / forwards.rs        |   |
|  +-----+-----------------------+----------------------------+   |
|        |                       |                                |
+--------+-----------------------+--------------------------------+
         |                       |
   +-----v-----+           +-----v--------------+
   | Unix sock |<--- MCP --| MCP server Python  |
   | (local)   |   CLI ----| CLI Rust (portsage)|
   +-----+-----+           +--------------------+
         |
         +-- BackendClient --> SQLite locale (default)
                          \-> SSH unix-socket tunnel --> portsage-server remoto (Linux)
```

Lo stesso binario Rust gira come GUI macOS (cargo feature `gui`, default) e come server headless Linux (`--no-default-features`, esclude tutta la toolchain Tauri). Su macOS il flag `--headless` sopprime tray e finestre ed espone solo il socket; cosi' la CLI fa autospawn del backend quando nessuna GUI e' in esecuzione.

### Moduli del backend Rust

| Modulo        | Responsabilita'                                                            |
|---------------|----------------------------------------------------------------------------|
| `paths.rs`    | Risoluzione path per OS (Application Support su macOS, XDG su Linux). Unico posto dove `dirs::*` e' ammesso. |
| `db.rs`       | Setup SQLite, migrazioni, tutta la CRUD. Condiviso via `Arc<Database>`. Vedi [DATABASE_SCHEMA.md](DATABASE_SCHEMA.md). |
| `actions.rs`  | Logica di dominio condivisa tra Tauri commands e socket dispatcher. Nessuna dipendenza Tauri. |
| `commands.rs` | Wrapper Tauri sottili su `actions::*` e `backends::*`. Unico layer Tauri. |
| `socket.rs`   | Server Unix socket (async). Parla il protocollo wire con MCP server, CLI e altri client. |
| `scanner.rs`  | Port scanner. Impl per-OS sotto `mod macos` (lsof + ps) e `mod linux` (`/proc/net/tcp` + fallback `ss`), selezionate da `#[cfg(target_os)]`. |
| `backends.rs` | `BackendTarget` / `BackendManager` (gestisce i tunnel SSH) / `BackendRouter` (target attivo) / `BackendClient` (adapter Local/Remote attraverso cui ogni Tauri command passa). Nessuna dipendenza Tauri. |
| `forwards.rs` | Fase 3 multi-host: `ForwardManager` gestisce lo stato per-(backend, porta) dei forward SSH locali. Tratti `ForwardController` + `LocalPortProbe` per testabilita'. Nessuna dipendenza Tauri. |

### MCP server

Thin client Python. Legge da stdio (transport Claude Code), inoltra le richieste JSON al socket Unix, restituisce la risposta. Nessun accesso DB diretto.

I quattro file sorgente (`server.py`, `pyproject.toml`, `uv.lock`, `SKILL.md`) sono embedded nel binario CLI via `include_str!` in `crates/portsage-cli/src/mcp.rs`. `portsage mcp install` li estrae in `<data_dir>/portsage/mcp/`, lancia `uv sync` e patcha `~/.claude.json` + `~/.claude/skills/portsage/` + `~/.claude/settings.json` atomicamente. La directory `mcp/` nel repo resta source of truth - ogni modifica li' richiede rebuild del CLI prima di shippare.

### CLI

`portsage` e' un binario clap in `crates/portsage-cli/`. Parla col backend via `portsage-client` (sync UnixStream), mai col DB direttamente. Auto-spawna il binario Tauri in `--headless` se nessuna istanza e' in esecuzione.

Per la lista completa dei sottocomandi vedi il [README](../README.md#cli).

## Path dei file

| Risorsa  | macOS                                                  | Linux                                                           |
|----------|--------------------------------------------------------|-----------------------------------------------------------------|
| Database | `~/Library/Application Support/portsage/portsage.db`   | `$XDG_DATA_HOME/portsage/portsage.db` (default `~/.local/share/portsage/`) |
| Socket   | `~/Library/Application Support/portsage/portsage.sock` | `$XDG_RUNTIME_DIR/portsage/portsage.sock` (fallback `/tmp/portsage-<uid>.sock`) |
| MCP dir  | `~/Library/Application Support/portsage/mcp/`          | `~/.local/share/portsage/mcp/`                                  |
| State    | come Application Support                               | `$XDG_STATE_HOME/portsage/` (default `~/.local/state/portsage/`) |

Il server headless accetta `--socket <path>` ed `PORTSAGE_SOCKET=<path>` per forzare la posizione del socket; l'unit systemd di sistema usa questo per metterlo in `/run/portsage/portsage.sock`.

La cartella padre del socket viene creata con mode `0700`, il file socket con `0600` per install per-utente e `0660` quando la padre e' gestita esternamente (install systemd di gruppo) - vedi `socket.rs` per il selettore.

## Multi-host

La storia single-host (UI Mac che parla a un backend locale) e' il default. L'estensione multi-host permette alla UI Mac di configurare server Portsage remoti e trattarli come backend di primo livello.

- **Fase 1 - Server headless Linux.** Build Linux x86_64 (`portsage-server`), scanner per-OS, path XDG, unit systemd + installer idempotente in `packaging/linux/`.
- **Fase 2 - Backend remoto in UI.** Tabella `remote_backends`, `BackendManager` con tunnel SSH unix-socket per backend (state machine + mutex per backend), `BackendRouter` con target corrente persistito. Ogni Tauri command passa per il `BackendClient` attivo. Il `BackendSwitcher` in sidebar mostra il pallino di stato live via eventi `tunnel://state-changed`.
- **Fase 3 - Auto SSH port forwarding.** Quando un backend remoto ha porte registrate, Portsage apre `ssh -O forward -L <port>:localhost:<port>` sullo stesso ControlMaster del tunnel di protocollo. Due modalita' di ownership del ControlMaster: piggyback sul `ControlMaster auto` del `ssh_config` utente (preferita; non viene mai chiusa), oppure apri un master Portsage-managed in `paths::state_dir()/cm-<alias>.sock`. La probe di collisione porte locali emette "port X is in use locally by node (pid 12345)" prima dell'`ssh`. Timer di sync periodico (60s) come safety-net per mutazioni remote.
- **Fase 4 - Rifinitura.** Migrazione progetti tra backend, dashboard di salute, CLI `portsage backends list / add / remove`, auto-discovery host Tailscale. Non iniziata.

Il piano completo + decisioni di design vivono in [multi-host-evolution.md](multi-host-evolution.md). Leggilo prima di toccare `scanner.rs`, `backends.rs`, `forwards.rs` o le parti multi-host di `db.rs`.

## Protocollo socket

Per la specifica completa (envelope JSON, metodi, tipi wire, esempio di sessione) fai riferimento alla [versione inglese](ARCHITECTURE.md#socket-protocol). I tipi canonici stanno in `crates/portsage-client/src/types.rs` e non vengono duplicati altrove.

## Database

Vedi [DATABASE_SCHEMA.md](DATABASE_SCHEMA.md) per il dettaglio tabella per tabella e la risoluzione dei path.

## Port scanning

- **macOS**: `lsof -iTCP -sTCP:LISTEN -nP`, nomi processo risolti con `ps -p <pid> -o comm=` per aggirare il troncamento di lsof.
- **Linux**: parsing di `/proc/net/tcp` (+ `/proc/net/tcp6`) e risoluzione del pid proprietario via readlink di `/proc/<pid>/fd/*`; fallback a `ss -tlnp` se `/proc` e' ristretto.

Le porte non gestite sono filtrate: porte >= 3000, esclusi processi noti di sistema (AirPlay, CUPS, mDNS, Spotlight, sshd, ecc.). La blocklist e' in `scanner.rs::is_system_process`.

## Stack

- **Frontend**: React 19 + TypeScript + Tailwind v4 (CSS-first via `@theme`).
- **App shell**: Tauri v2 (Rust) - menubar, system tray, popover, finestra app.
- **MCP server**: Python + FastMCP SDK (thin client via stdio).
- **CLI**: Rust + clap, sync UnixStream client.
- **Database**: SQLite (vedi [DATABASE_SCHEMA.md](DATABASE_SCHEMA.md)).
- **Package manager**: pnpm (frontend), uv (Python), cargo (workspace Rust).
- **Font**: system-ui (UI), ui-monospace (titoli e dati tecnici).

## Distribuzione

- **Dev**: `pnpm tauri dev`.
- **Build**: `pnpm tauri build` (genera `.dmg`).
- **Homebrew**: `brew tap essedev/portsage && brew install portsage`.
- **Server Linux**: tarball dalla release GitHub; `packaging/linux/install.sh` installa binario + unit systemd, riscrivendo `User=`/`Group=` al dev user (il kernel via `__ptrace_may_access(PTRACE_MODE_FSCREDS)` pretende che `fsuid` E `fsgid` matchino i creds del processo target per i readlink di `/proc/<pid>/fd`).
- **Processo di release**: vedi [RELEASING.md](RELEASING.md).
- **GitHub**: <https://github.com/essedev/portsage>.

## Integrazione MCP

Il MCP server espone 15 tool in tre gruppi (parità completa con la CLI):

- **Lettura**: `list_all`, `scan_active`, `list_unmanaged`, `next_range`, `get_config`, `find_project_by_path`.
- **Mutazione**: `reserve_range`, `update_project`, `register_port`, `remove_port`, `release_project`, `set_config`.
- **Azione**: `kill_port`, `kill_project`, `open_in_browser`.

**Claude Code**: install via `portsage mcp install` (canonico, funziona senza GUI in esecuzione) oppure dall'app (Impostazioni > "Configura MCP" > Claude Code). `portsage mcp uninstall` e `portsage mcp status` completano il ciclo. Le patch a `~/.claude.json` / `~/.claude/skills/portsage/` / `~/.claude/settings.json` passano da un helper parse-or-bail + atomic-tmp-then-rename - un config corrotto non viene mai sovrascritto silenziosamente.

**Altri editor**: l'app genera config pronta per Cursor, Claude Desktop, Cline, VS Code (Copilot), Codex (TOML), Windsurf. Impostazioni > "Configura MCP" > "Altri editor" mostra il dropdown editor + snippet generato.

## Strategia di test

I test coprono il **core di dominio** (allocazione porte, parsing lsof / `/proc/net/tcp`, protocollo socket, humanizzazione errori, JSON-merge safety per i config editor) e gli **helper safety-critical**. Non coprono wrapper sottili, plumbing del framework o rendering UI.

- Test Rust inline sotto `#[cfg(test)] mod tests` in ogni modulo; SQLite in-memory via `Database::in_memory()`. Lancia il workspace con `cargo test` dalla root.
- Test frontend accanto ai sorgenti come `*.test.ts`, lanciati via `vitest`.
- Protocollo socket: preferisci test end-to-end che spawnano un `UnixListener` reale ed esercitano `portsage_client::Client` su `handle_request` (vedi `socket.rs::end_to_end_round_trip_via_real_client`) - cosi' becchi le derive tra deserializer client e shape della risposta server.
- Race condition in `Database::create_project`: regression test dedicato (`concurrent_create_project_produces_no_overlapping_ranges`).

## UI

### Popover menubar

Pannello compatto (350x480px), solo lettura, per check veloce.

- Header: titolo "portsage" con glow ambra + badge porte attive + icona Quit.
- Lista progetti con porte e stato attivo (pallino ambra/grigio).
- Click sul numero di porta apre `http://localhost:PORTA` nel browser predefinito.
- Footer con conteggio porte e link "Apri portsage".

### Finestra full

Ridimensionabile (min 700x400) con titlebar trasparente.

**Header**
- Titolo "portsage" con glow ambra + tagline "ports under control".
- Badge porte attive.

**Sidebar**
- Dropdown `BackendSwitcher` (Local / alias SSH remoti) con pallino di stato live.
- Campo ricerca/filtro.
- Bottone "Nuovo progetto".
- Lista progetti con badge porte attive (nome progetto attivo in `text-primary`, inattivo in `text-secondary`).
- Sezione "Non gestite" (conteggio, visibile solo se presenti).
- Bottone "Configura MCP" (visibile solo se MCP non installato per Claude Code).
- Bottone "Impostazioni".

**Welcome panel** (nessuna selezione)
- Prima esecuzione: tagline + CTA "Crea il tuo primo progetto" + checklist "Next steps" con deep-link a Impostazioni > Integrazioni.
- Con progetti: stat card (Progetti / Porte registrate / Attive) + bottoni rapidi.

**Dettaglio progetto**
- Nome, path, range assegnato, badge porte attive.
- Lista servizi: nome servizio, processo, PID, porta, stato attivo.
- Click sul numero di porta apre `http://localhost:PORTA` nel browser predefinito.
- Bottone Power per singola porta (warning/ambra): chiede conferma e invia SIGTERM, escalation a SIGKILL dopo 2s.
- Aggiunta servizi con dropdown porte libere nel range.
- Toolbar split: azioni di navigazione (Finder / Terminale - nascoste se il backend attivo e' Remote) a sinistra, azioni distruttive (Stop tutte / Elimina progetto) a destra.
- Indicatori di forward Fase 3: freccia accanto a ogni porta remota - ambra (forwarded), rossa (failed, hover per il motivo), spenta (cancellata, click per riaprire).

**Porte non gestite**
- Tabella con porta, nome processo, PID.
- Click sul numero di porta apre `http://localhost:PORTA` nel browser predefinito.
- Bottone Power per riga (warning/ambra).
- Solo porte >= 3000, filtrate da processi di sistema.

**Impostazioni** (tab)
- **Generali**: avvio al login, base_port + range_size.
- **Integrazioni**: MCP Claude Code (stato, install/rimuovi, lista tool) + Altri editor (accordion, chiuso di default).
- **Backend remoti**: lista, add/test/rimuovi, toggle auto-forward, sub-UI "Porte escluse" per backend.
- **Dati**: export/import (`.portsage` = zip con DB + config).

### Icona menubar

Icona template (si adatta al dark/light mode di macOS).

## Convenzioni

Le convenzioni specifiche del progetto (import ammessi/vietati, line length, mappa "cosa sta dove") vivono nel [CLAUDE.md](../CLAUDE.md) in root. Design tokens, tipografia, spaziature e component library in [DESIGN.md](DESIGN.md).
