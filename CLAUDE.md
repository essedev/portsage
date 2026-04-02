# Grimport - Convenzioni progetto

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
    ui/           # Primitivi (GrimCard, GrimButton, etc.)
    ProjectCard.tsx
    ProjectList.tsx
    ...           # Componenti composti
  features/
    projects/     # Logica progetti (hooks, types)
    ports/        # Logica porte
    settings/     # Logica settings
  lib/            # Utility, Tauri commands wrapper
  App.tsx
  main.tsx
  index.css       # Token CSS custom (@theme Tailwind v4)
src-tauri/
  src/
    lib.rs        # Entry point Tauri
    db.rs         # SQLite setup e queries
    commands.rs   # Tauri commands (IPC frontend-backend)
    scanner.rs    # Port scanner (lsof wrapper)
    socket.rs     # Unix socket server per MCP
  migrations/     # SQL migrations
mcp/              # MCP server Python (thin client)
```

## Regole

### Frontend
- Tutti i colori via CSS token definiti in DESIGN.md, mai hardcoded
- Spacing solo con i token --space-N, border-radius con --radius-*
- Componenti `Grim*` in `src/components/ui/`, composti in `src/components/`
- Dipendenza unidirezionale: primitivi <- composti. I `Grim*` non importano composti
- Props tipizzate con interface nello stesso file del componente
- Tailwind v4: CSS-first config con @theme, niente tailwind.config.ts
- Font pixel medievale solo per varianti title e section di GrimText
- Import alias: `@/` per import assoluti

### Rust backend
- Tutto il DB access in db.rs, esposto ai frontend via commands.rs
- Port scanning in scanner.rs, non mischiare con logica DB
- Unix socket in socket.rs, gestisce richieste MCP
- Errori tipizzati, no unwrap() in produzione

### MCP server
- Thin client: nessun accesso diretto al DB
- Comunica solo via Unix socket con il Rust backend

### Generali
- Codice in inglese, UI in italiano
- Line length: 100 caratteri
- pnpm per frontend, uv per Python
