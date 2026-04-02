# Design System

Visual identity e component library per Grimport. Vibe: terminale di un mago - minimal, dark, tocchi di magia senza essere pacchiano.

## Palette

### Base

| Token | Hex | Uso |
|-------|-----|-----|
| `--bg-deep` | `#1a1a1f` | Sfondo principale finestra |
| `--bg-surface` | `#222228` | Card, sidebar, popover |
| `--bg-elevated` | `#2a2a32` | Hover, elementi in rilievo |
| `--bg-input` | `#1e1e24` | Campi input, search |
| `--border-subtle` | `#333340` | Bordi default |
| `--border-glow` | `#d4a04420` | Bordi con glow ambra (20% opacita') |

### Testo

| Token | Hex | Uso |
|-------|-----|-----|
| `--text-primary` | `#e8e4df` | Testo principale (caldo, non bianco puro) |
| `--text-secondary` | `#9a9590` | Label, info secondarie |
| `--text-muted` | `#5c5856` | Placeholder, hint |

### Accenti

| Token | Hex | Uso |
|-------|-----|-----|
| `--accent-amber` | `#d4a044` | Porte attive, azioni primarie, titoli sezione |
| `--accent-amber-glow` | `#d4a04440` | Glow/shadow ambra |
| `--accent-amber-soft` | `#d4a04415` | Background badge attivo |
| `--status-inactive` | `#4a4550` | Porte inattive |
| `--status-inactive-text` | `#6b6570` | Testo porte inattive |
| `--accent-danger` | `#c45c5c` | Rimozione, collisioni |
| `--accent-danger-soft` | `#c45c5c15` | Background badge errore |

## Tipografia

### Font

- **Titoli e sezioni**: Pixelify Sans - pixel medievale leggibile, non troppo pesante
- **UI e body**: `Inter` - pulito, ottima leggibilita' a tutte le dimensioni
- **Dati tecnici** (porte, range, path): `JetBrains Mono` - monospace

### Scale

| Token | Size | Weight | Font | Uso |
|-------|------|--------|------|-----|
| `--text-title` | 18px | 400 | Pixelify Sans | "Grimport" nell'header |
| `--text-section` | 13px | 400 | Pixelify Sans | Nomi progetti, titoli sezione |
| `--text-body` | 13px | 400 | Inter | Testo UI generale |
| `--text-label` | 11px | 500 | Inter | Label, badge |
| `--text-mono` | 12px | 400 | JetBrains Mono | Porte, range, path |

## Spacing

### Scale

Base 4px. Usare solo questi valori per padding, margin, gap.

| Token | Value | Uso tipico |
|-------|-------|------------|
| `--space-1` | 4px | Gap tra icona e testo, padding badge |
| `--space-2` | 8px | Padding interno piccolo, gap tra elementi inline |
| `--space-3` | 12px | Padding card, gap tra righe lista |
| `--space-4` | 16px | Padding sezioni, gap tra card |
| `--space-5` | 20px | Padding pannelli principali |
| `--space-6` | 24px | Margine tra sezioni |
| `--space-8` | 32px | Spacing grandi (header, footer) |

### Border radius

| Token | Value | Uso |
|-------|-------|-----|
| `--radius-sm` | 4px | Badge, bottoni piccoli, input |
| `--radius-md` | 8px | Card, pannelli, tooltip |
| `--radius-lg` | 12px | Finestra popover, modali |

### Layout

| Elemento | Dimensione |
|----------|-----------|
| Popover | 350px larghezza, max 480px altezza |
| Sidebar finestra full | 240px larghezza |
| Porte status dot | 8px diametro |
| Icone UI | 16px (small), 20px (default) |
| Altezza riga PortRow | 32px |
| Altezza riga ProjectCard header | 40px |

## Effetti

### Glow ambra

Usato con parsimonia su elementi chiave: bordi card hover, porte attive, azioni primarie.

```css
/* Bordo glow */
box-shadow: 0 0 8px var(--accent-amber-glow);
border-color: var(--accent-amber);

/* Testo glow (solo titolo app) */
text-shadow: 0 0 12px var(--accent-amber-glow);
```

### Transizioni

- Default: `150ms ease` per hover, colori
- Glow: `200ms ease-in-out` per comparsa/scomparsa glow
- Niente animazioni complesse o gratuite

## Iconografia

- Stile: line-art, stroke 1.5px, colore `--text-secondary`
- Hover: colore `--text-primary`
- Attivo/selected: colore `--accent-amber`
- Libreria: Lucide icons (gia' ottima con shadcn)

## Component Library

Tutti i componenti in `src/components/ui/` (base) e `src/components/` (composti).

### Primitivi (src/components/ui/)

Componenti base riutilizzabili, senza logica di dominio.

| Componente | Descrizione |
|------------|-------------|
| `GrimText` | Wrapper tipografico. Props: `variant` (title, section, body, label, mono), renderizza con font e size corretti |
| `GrimCard` | Container con bg-surface, bordo subtle, hover con glow opzionale |
| `GrimBadge` | Badge piccolo. Varianti: `active` (ambra), `inactive` (grigio), `danger` (rosso) |
| `GrimStatus` | Pallino stato porta. Props: `active: boolean` - ambra con glow o grigio spento |
| `GrimButton` | Bottone. Varianti: `primary` (ambra), `ghost` (trasparente), `danger` (rosso) |
| `GrimInput` | Campo input con sfondo bg-input, bordo subtle, focus con bordo ambra |
| `GrimSearch` | Input con icona search, specifico per filtro |
| `GrimSelect` | Dropdown custom (no select nativo). Props: `options`, `value`, `onChange`. Chevron animato, lista con hover/selected states |
| `GrimDivider` | Linea separatrice orizzontale, colore border-subtle |

### Composti (src/components/)

Componenti che combinano primitivi con logica di dominio.

| Componente | Descrizione |
|------------|-------------|
| `PortRow` | Riga singola porta: nome servizio + porta (mono) + GrimStatus |
| `ProjectCard` | Card progetto: nome (section font) + range (mono) + badge porte attive + lista PortRow |
| `ProjectList` | Lista scrollabile di ProjectCard con GrimSearch in cima |
| `ProjectDetail` | Pannello dettaglio: info progetto + lista servizi + azioni |
| `PopoverPanel` | Layout popover completo: header + ProjectList compatta + footer |
| `Sidebar` | Sidebar finestra full: lista progetti compatta + search + bottone aggiungi |
| `AppHeader` | Header con titolo "grimport" (title font + glow) + tagline + badge porte attive |
| `SettingsPanel` | Pannello settings: autostart, config porte, integrazione Claude Code, export/import |
| `UnmanagedPortsPanel` | Tabella porte attive non associate a progetti (porta, processo, PID) |
| `AddProjectForm` | Form aggiunta progetto: nome, path, conferma |
| `AddPortForm` | Form aggiunta servizio + dropdown porte libere nel range |

### Convenzioni componenti

- Ogni componente in un file singolo: `src/components/ui/GrimCard.tsx`, `src/components/ProjectCard.tsx`
- Props tipizzate con interface dedicata nello stesso file
- Styling con Tailwind classes, token CSS custom per i colori del design system
- Nessun componente puo' usare colori hardcoded - sempre tramite token
- I componenti `Grim*` non importano mai componenti composti (dipendenza unidirezionale)
- I componenti composti possono importare solo `Grim*` e altri composti
