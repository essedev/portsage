# Roadmap

## v0.1 - Foundation

- [x] Setup progetto Tauri v2 + React 19 + Tailwind v4
- [x] SQLite database: schema, migrations, path `~/.config/grimport/`
- [x] Rust backend: CRUD progetti e porte, accesso DB
- [x] Port scanner: wrapper su `lsof` per rilevare porte attive
- [x] Menubar icon + popover con lista progetti e stato porte
- [x] Finestra app full: sidebar progetti + dettaglio con servizi/porte
- [x] Aggiunta/rimozione progetti e servizi dalla UI

## v0.2 - MCP server

- [x] Unix socket locale esposto dal Rust backend (`~/.config/grimport/grimport.sock`)
- [x] MCP server Python (thin client, inoltra al socket)
- [x] Tool: `list_all` - registry completo + stato porte
- [x] Tool: `reserve_range(project_name)` - riserva prossimo range libero
- [x] Tool: `register_port(project_name, service, port)` - registra porta
- [x] Tool: `release_project(project_name)` - libera range
- [x] Tool: `scan_active` - porte attive sulla macchina
- [x] Skill file per Claude Code
- [x] Script install + UI "Connetti a Claude Code" nelle impostazioni

## v0.3 - Polish

- [x] Auto-refresh stato porte (polling ogni 5s)
- [x] Nome processo visibile accanto alle porte attive (risolto via `ps`)
- [x] Ricerca/filtro progetti nella sidebar
- [x] Porte non gestite: rileva porte attive > 3000 non associate a progetti
- [x] Click su path progetto -> apre nel Finder o Terminale

## v0.4 - Settings e portabilita'

- [x] Settings: configurazione base_port e range_size
- [x] Export: DB + preferenze in file `.grimport` (zip con SQLite dump + config)
- [x] Import: ripristino da file `.grimport`
- [x] Launch at login (tauri-plugin-autostart)
- [ ] ~~Import porte da docker-compose.yml~~ (coperto dal MCP)
- [ ] ~~Dark/light mode~~ (il tema dark e' l'identita' dell'app)

## v0.5 - Distribuzione

- [x] Build `.dmg` con `tauri build`
- [x] Homebrew tap (`brew tap essedev/grimport && brew install grimport`)
- [x] GitHub release con DMG allegato
- [ ] Auto-update (Tauri updater) - futuro
