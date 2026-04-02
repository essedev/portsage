# Roadmap

## v0.1 - Foundation

- [ ] Setup progetto Tauri v2 + React 19 + Tailwind v4
- [ ] SQLite database: schema, migrations, path `~/.config/grimport/`
- [ ] Rust backend: CRUD progetti e porte, accesso DB
- [ ] Port scanner: wrapper su `lsof` per rilevare porte attive
- [ ] Menubar icon + popover con lista progetti e stato porte (verde/rosso)
- [ ] Finestra app full: sidebar progetti + dettaglio con servizi/porte
- [ ] Aggiunta/rimozione progetti e servizi dalla UI

## v0.2 - MCP server

- [ ] Unix socket locale esposto dal Rust backend (`~/.config/grimport/grimport.sock`)
- [ ] MCP server Python (thin client, inoltra al socket)
- [ ] Tool: `list_all` - registry completo + stato porte
- [ ] Tool: `reserve_range(project_name)` - riserva prossimo range libero
- [ ] Tool: `register_port(project_name, service, port)` - registra porta
- [ ] Tool: `release_project(project_name)` - libera range
- [ ] Tool: `scan_active` - porte attive sulla macchina
- [ ] Regola per CLAUDE.md globale
- [ ] Documentazione setup MCP in Claude Code

## v0.3 - Polish

- [ ] Auto-refresh stato porte (polling ogni 5s)
- [ ] Notifica se una porta registrata collide con un processo esterno
- [ ] Ricerca/filtro progetti nel pannello e nella finestra full
- [ ] Badge numerico nell'icona menubar (progetti attivi)
- [ ] Click su path progetto -> apre nel terminale/editor

## v0.4 - Settings e portabilita'

- [ ] Settings: configurazione base_port e range_size
- [ ] Export: DB + preferenze in file `.grimport` (zip con SQLite dump + config)
- [ ] Import: ripristino da file `.grimport`
- [ ] Import porte da docker-compose.yml esistenti
- [ ] Dark/light mode (segue sistema)
- [ ] Launch at login

## v0.5 - Distribuzione

- [ ] Build `.dmg` con `tauri build`
- [ ] Homebrew tap per installazione
- [ ] Auto-update (Tauri updater)
