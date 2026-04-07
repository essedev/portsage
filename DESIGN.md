# Design System

Visual identity and component library for Portsage. Vibe: a wizard's terminal - minimal, dark, touches of magic without being kitsch.

## Palette

### Base

| Token | Hex | Use |
|-------|-----|-----|
| `--bg-deep` | `#1a1a1f` | Main window background |
| `--bg-surface` | `#222228` | Cards, sidebar, popover |
| `--bg-elevated` | `#2a2a32` | Hover, raised elements |
| `--bg-input` | `#1e1e24` | Input fields, search |
| `--border-subtle` | `#333340` | Default borders |
| `--border-glow` | `#d4a04420` | Borders with amber glow (20% opacity) |

### Text

| Token | Hex | Use |
|-------|-----|-----|
| `--text-primary` | `#e8e4df` | Main text (warm, not pure white) |
| `--text-secondary` | `#9a9590` | Labels, secondary info |
| `--text-muted` | `#5c5856` | Placeholders, hints |

### Accents

| Token | Hex | Use |
|-------|-----|-----|
| `--accent-amber` | `#d4a044` | Active ports, primary actions, section titles |
| `--accent-amber-glow` | `#d4a04440` | Amber glow/shadow |
| `--accent-amber-soft` | `#d4a04415` | Active badge background |
| `--status-inactive` | `#4a4550` | Inactive ports |
| `--status-inactive-text` | `#6b6570` | Inactive ports text |
| `--accent-danger` | `#c45c5c` | Removal, collisions |
| `--accent-danger-soft` | `#c45c5c15` | Error badge background |

## Typography

### Fonts

- **Titles and sections**: `ui-monospace` (system monospace) - matches the terminal vibe
- **UI and body**: `system-ui` (system sans-serif) - clean, very readable
- **Technical data** (ports, ranges, paths): `ui-monospace` (system monospace)

### Scale

| Token | Size | Weight | Font | Use |
|-------|------|--------|------|-----|
| `--text-title` | 18px | 400 | ui-monospace | "Portsage" in the header |
| `--text-section` | 13px | 400 | ui-monospace | Project names, section titles |
| `--text-body` | 13px | 400 | Inter | General UI text |
| `--text-label` | 11px | 500 | Inter | Labels, badges |
| `--text-mono` | 12px | 400 | JetBrains Mono | Ports, ranges, paths |

## Spacing

### Scale

Base 4px. Use only these values for padding, margin, gap.

| Token | Value | Typical use |
|-------|-------|------------|
| `--space-1` | 4px | Gap between icon and text, badge padding |
| `--space-2` | 8px | Small inner padding, gap between inline elements |
| `--space-3` | 12px | Card padding, gap between list rows |
| `--space-4` | 16px | Section padding, gap between cards |
| `--space-5` | 20px | Padding for main panels |
| `--space-6` | 24px | Margin between sections |
| `--space-8` | 32px | Large spacing (header, footer) |

### Border radius

| Token | Value | Use |
|-------|-------|-----|
| `--radius-sm` | 4px | Badges, small buttons, inputs |
| `--radius-md` | 8px | Cards, panels, tooltips |
| `--radius-lg` | 12px | Popover window, modals |

### Layout

| Element | Size |
|----------|-----------|
| Popover | 350px wide, max 480px tall |
| Full window sidebar | 240px wide |
| Port status dot | 8px diameter |
| UI icons | 16px (small), 20px (default) |
| PortRow row height | 32px |
| ProjectCard header row height | 40px |

## Effects

### Amber glow

Used sparingly on key elements: card hover borders, active ports, primary actions.

```css
/* Glow border */
box-shadow: 0 0 8px var(--accent-amber-glow);
border-color: var(--accent-amber);

/* Text glow (only the app title) */
text-shadow: 0 0 12px var(--accent-amber-glow);
```

### Transitions

- Default: `150ms ease` for hover, colors
- Glow: `200ms ease-in-out` for glow appear/disappear
- No complex or gratuitous animations

## Iconography

- Style: line-art, 1.5px stroke, color `--text-secondary`
- Hover: color `--text-primary`
- Active/selected: color `--accent-amber`
- Library: Lucide icons (already great with shadcn)

## Component Library

All components live in `src/components/ui/` (base) and `src/components/` (composed).

### Primitives (src/components/ui/)

Reusable base components, no domain logic.

| Component | Description |
|------------|-------------|
| `UIText` | Typographic wrapper. Props: `variant` (title, section, body, label, mono); renders with the right font and size |
| `UICard` | Container with bg-surface, subtle border, optional glow on hover |
| `UIBadge` | Small badge. Variants: `active` (amber), `inactive` (grey), `danger` (red) |
| `UIStatus` | Port status dot. Props: `active: boolean` - amber with glow or quiet grey |
| `UIButton` | Button. Variants: `primary` (amber), `ghost` (transparent), `danger` (red) |
| `UIInput` | Input field with bg-input, subtle border, amber border on focus |
| `UISearch` | Input with search icon, used for filters |
| `UISelect` | Custom dropdown (no native select). Props: `options`, `value`, `onChange`. Animated chevron, list with hover/selected states |
| `UIDivider` | Horizontal separator line, color border-subtle |

### Composed (src/components/)

Components that combine primitives with domain logic.

| Component | Description |
|------------|-------------|
| `PortRow` | Single port row: service name + port (mono) + UIStatus |
| `ProjectCard` | Project card: name (section font) + range (mono) + active-ports badge + PortRow list |
| `ProjectList` | Scrollable list of ProjectCards with UISearch on top |
| `ProjectDetail` | Detail panel: project info + service list + actions |
| `PopoverPanel` | Full popover layout: header + compact ProjectList + footer |
| `Sidebar` | Full window sidebar: compact project list + search + add button |
| `AppHeader` | Header with the "portsage" title (title font + glow) + tagline + active-ports badge |
| `SettingsPanel` | Settings panel: autostart, port config, Claude Code integration, export/import |
| `UnmanagedPortsPanel` | Table of active ports not associated with projects (port, process, PID) |
| `AddProjectForm` | Add-project form: name, path, confirm |
| `AddPortForm` | Add-service form + dropdown of free ports in the range |

### Component conventions

- Each component lives in a single file: `src/components/ui/UICard.tsx`, `src/components/ProjectCard.tsx`
- Props typed with a dedicated interface in the same file
- Styling with Tailwind classes, custom CSS tokens for design system colors
- No component may use hardcoded colors - always via tokens
- `UI*` components never import composed components (one-way dependency)
- Composed components may only import `UI*` and other composed components
