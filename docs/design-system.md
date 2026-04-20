# Grumps design system

Warm brutalism. Slab serif for display, DM Sans for body. Cream + ink
identity with brick, teal, ochre accents. 2px angular borders, grain
overlay, no shadows beyond the offset-block signature. Works in both
light and dark modes via semantic tokens. Voice is terse and dry.

Source of truth for decisions: `docs/superpowers/specs/2026-04-20-design-system-design.md`.
This document is the working reference — read it before touching UI.

## Tokens

### Primitives (raw palette)
`--cream`, `--cream-light`, `--ink`, `--ink-70/40/15/08`, `--brick` (+hover),
`--teal` (+light), `--ochre` (+light), `--warm-gray` (+light).

### Semantic (use these in new code)
| Token | Role |
|---|---|
| `--surface-base` | Canvas / page background |
| `--surface-raised` | Cards, sidebar |
| `--text-primary` | Main text |
| `--text-secondary` | Inactive nav, secondary copy |
| `--text-muted` | Meta labels, placeholders |
| `--border-strong` | 2px container borders |
| `--border-subtle` | Internal separators |
| `--hover-tint` | Hover overlay |
| `--accent-primary` | Brick (urgency, active) |
| `--accent-primary-hover` | Hover variant |
| `--accent-success` / `--accent-success-bg` | Teal |
| `--accent-warning` / `--accent-warning-bg` | Ochre |

Tailwind utilities: `bg-surface`, `bg-surface-raised`, `text-primary`,
`text-secondary`, `text-muted`, `border-strong`, `border-subtle`,
`bg-hover-tint`, `bg-accent`, `text-accent`, etc.

### Dark mode
Warm-dark canvas (`#1A1915`), cream text, accents brightened for
contrast. Auto-applied via `[data-theme="dark"]` on `<html>`.
User toggles via the sidebar (Light / Auto / Dark); persisted in
`localStorage.theme`.

### Typography
Use the named scale — **never** `text-[Npx]`:
`text-display-xl` (40px hero) · `text-display-lg` (28px page title) ·
`text-display` (20px card/section) · `text-body` (15px) ·
`text-body-sm` (13px) · `text-meta` (11px uppercase) ·
`text-eyebrow` (10px uppercase).

### Borders & radius
- 2px strong on containers, 1px subtle on separators, 1px on badges.
- Radius always `rounded-sm` (= 3px). Never `rounded-[3px]` in source.

### Offset-block shadow
`box-shadow: 3px 3px 0 var(--border-strong)`. Hero / modal surfaces
only. 1–2 instances per page max. Never on list-item cards.

## Icons

Monochrome SVG only. No emojis in UI chrome, ever.
Use `<Icon name="..." class="size-4"/>`.
Available names: overview, todos, notes, files, history, calendar,
memory, scheduled, settings, workspaces, globe, message, recap,
task-check, webhook, bolt, pin, chevron-down.

Adding an icon: draw 24×24, 2px stroke, square caps, integer coords.
Save to `crates/spa/assets/icons/<name>.svg`. Add a match arm in
`crates/spa/src/components/icon.rs`. Append name above.

## Components (rules, not templates)

**Button.** Primary = ink bg + cream text + 2px ink border + uppercase
`text-meta`. Secondary = cream-light + ink text. Ghost = no border,
underline on hover. Destructive = brick bg. Never lowercase.

**Input.** 2px ink border, cream-light bg, focus → brick border. No
browser blue ring.

**Card (listed).** 2px ink border, rounded-sm, surface-raised bg,
`text-display` title, `text-meta` secondary. No offset-block.

**Nav item.** 3px left border (transparent → brick when active),
`text-body-sm text-secondary`, hover tint overlay.

**Badge.** 1px ink border, `text-meta`. Status variants use
`--accent-success-bg` / `--accent-warning-bg` / `--accent-primary`.

**Empty state.** `<Icon size-6>` above, `text-display` title,
`text-body-sm text-muted` message, optional secondary CTA.

## Voice

- Short sentences. No exclamation marks. No emojis. No "please".
- Imperative form for CTAs and actions.
- Errors: factual, actionable. ("Invalid phone number.")
- Empty states: brief. ("Nothing yet.")
- Landing (`index.html`) may relax slightly for warmth — longer
  sentences OK, but still no `!`, no emojis, no SaaS superlatives.

## Enforcement

- Read this document before any frontend change.
- Grep the codebase after refactors: no emojis in UI, no `text-[Npx]`,
  no `rounded-[3px]`, no `!` in i18n strings.
- Skill: `grumps-design` auto-triggers on frontend file changes and
  forces a read of this doc.
