# grimport - Convenzioni progetto

## Panoramica

Menubar app macOS per gestione allocazione porte tra progetti. Vedi PROJECT.md per architettura, DESIGN.md per design system, ROADMAP.md per roadmap.

## Comandi

- `pnpm tauri dev` - avvia app in dev
- `pnpm dev` - solo frontend (Vite)
- `pnpm build` - build frontend
- `pnpm tauri build` - build app completa (.dmg)

## Struttura

```
src/
  components/
    ui/               # Primitivi (GrimCard, GrimButton, GrimSelect, etc.)
    PortRow.tsx
    ProjectCard.tsx
    ProjectDetail.tsx
    ProjectList.tsx
    PopoverPanel.tsx
    Sidebar.tsx
    AppHeader.tsx
    SettingsPanel.tsx
    UnmanagedPortsPanel.tsx
    AddProjectForm.tsx
    AddPortForm.tsx
  features/
    projects/         # useProjects hook
  lib/
    commands.ts       # Wrapper Tauri invoke commands
    types.ts          # TypeScript types (ProjectStatus, PortStatus, etc.)
  App.tsx             # Routing finestra main vs popover
  main.tsx
  index.css           # Token CSS custom (@theme Tailwind v4)
src-tauri/
  src/
    lib.rs            # Entry point Tauri, tray icon, popover logic
    db.rs             # SQLite setup, migrations, CRUD
    commands.rs       # Tauri commands (IPC frontend-backend)
    scanner.rs        # Port scanner (lsof + ps), unmanaged ports, blocklist
    socket.rs         # Unix socket server per MCP
  capabilities/
    default.json      # Permissions per plugins (dialog, autostart, opener)
  tauri.conf.json     # Config app, finestre, tray icon, bundle
mcp/
  server.py           # MCP server Python (thin client via stdio)
  SKILL.md            # Skill file per Claude Code
  install.sh          # Script installazione da terminale
  pyproject.toml      # Dipendenze Python
homebrew/
  grimport.rb         # Homebrew cask template
```

## Regole

### Frontend
- Tutti i colori via CSS token definiti in DESIGN.md, mai hardcoded
- Spacing solo con i token --space-N, border-radius con --radius-*
- Componenti `Grim*` in `src/components/ui/`, composti in `src/components/`
- Dipendenza unidirezionale: primitivi <- composti. I `Grim*` non importano composti
- Props tipizzate con interface nello stesso file del componente
- Tailwind v4: CSS-first config con @theme, niente tailwind.config.ts
- Font Pixelify Sans solo per varianti title e section di GrimText
- Import alias: `@/` per import assoluti
- Dropdown custom (GrimSelect), mai select nativo

### Rust backend
- Tutto il DB access in db.rs, esposto ai frontend via commands.rs
- Port scanning in scanner.rs, non mischiare con logica DB
- Unix socket in socket.rs, gestisce richieste MCP
- Database condiviso via Arc<Database> tra Tauri state e socket server
- Errori tipizzati, no unwrap() in produzione

### MCP server
- Thin client: nessun accesso diretto al DB
- Comunica solo via Unix socket con il Rust backend
- Trasporto stdio per integrazione con Claude Code

### Generali
- Codice in inglese, UI in italiano
- Line length: 100 caratteri
- pnpm per frontend, uv per Python
