# Design system — formalization & alignment

**Date**: 2026-04-20
**Status**: Approved design, pending implementation plan

## Purpose

The Grumps UI has a strong warm-brutalist aesthetic already documented in
`CLAUDE.md` and realized in `index.html`, `workspace.html`, and
`crates/spa/`. But the system drifts in practice: coloured emojis mix
with monochrome Unicode symbols in the sidebar; text sizes are picked
ad hoc (`text-[10px]`, `text-[11px]`, `text-[13px]`, `text-[15px]`);
the offset-block shadow is used inconsistently; `rounded-[3px]` and
`rounded-sm` coexist for the same radius.

This spec formalizes the design system based on what already works,
with improvements only where drift is systemic. It defines a canonical
living document, a skill to enforce it, and an audit of current
violations that a follow-up implementation plan will clean up.

## Non-goals

- Redesigning the visual identity (warm brutalism stays as-is).
- Animations / motion system — left for a later spec if needed.
- Dark mode for messaging surfaces (bot replies, email templates) —
  scoped to web surfaces only.

## 1. Tokens

### Palette (unchanged)

Existing CSS custom properties in `index.html` `:root` are the canonical
primitive palette:

| Token | Value | Use |
|---|---|---|
| `--cream` | `#F5F0E8` | Base page background |
| `--cream-light` | `#FAF8F4` | Card surfaces, sidebar bg |
| `--ink` | `#1A1915` | Text, borders, icons |
| `--ink-70` | `rgba(26,25,21,0.7)` | Secondary text, inactive nav |
| `--ink-40` | `rgba(26,25,21,0.4)` | Meta labels, placeholders |
| `--ink-15` | `rgba(26,25,21,0.15)` | Subtle separators |
| `--ink-08` | `rgba(26,25,21,0.08)` | Hover tint |
| `--brick` | `#C0392B` | Urgency, destructive, active accent |
| `--brick-hover` | `#A93226` | Primary button hover |
| `--teal` | `#1B6B5A` | Success, positive state |
| `--teal-light` | `#E8F5F0` | Success badge bg |
| `--ochre` | `#D4940A` | Warning |
| `--ochre-light` | `#FFF8E7` | Warning badge bg |
| `--warm-gray` | `#D5CFC3` | Neutral surface |
| `--warm-gray-light` | `#E8E4DB` | Secondary button hover |

No additions, no removals on the light palette.

### Dark palette (new)

Dark mode is a **warm dark**, not an inversion. The aesthetic stays:
slab serif, 2px borders, grain, the same accents. The canvas becomes
the existing `--ink` colour reused as a room-at-night background.

| Semantic role | Light value | Dark value |
|---|---|---|
| Canvas / page bg | `#F5F0E8` (`--cream`) | `#1A1915` (reuses `--ink`) |
| Raised surface (cards, sidebar) | `#FAF8F4` (`--cream-light`) | `#221F19` |
| Primary text | `#1A1915` (`--ink`) | `#F5F0E8` (`--cream`) |
| Text-secondary | `rgba(26,25,21,0.7)` | `rgba(245,240,232,0.75)` |
| Text-muted | `rgba(26,25,21,0.4)` | `rgba(245,240,232,0.5)` |
| Separator subtle | `rgba(26,25,21,0.15)` | `rgba(245,240,232,0.18)` |
| Hover tint | `rgba(26,25,21,0.08)` | `rgba(245,240,232,0.08)` |
| Strong border | `#1A1915` | `rgba(245,240,232,0.88)` |
| Brick (accent) | `#C0392B` | `#E04A38` (brighter for contrast) |
| Brick hover | `#A93226` | `#CB3E2E` |
| Teal | `#1B6B5A` | `#3FA58F` |
| Teal-light (bg) | `#E8F5F0` | `#223A33` |
| Ochre | `#D4940A` | `#E8AA2C` |
| Ochre-light (bg) | `#FFF8E7` | `#3A2F14` |
| Warm-gray | `#D5CFC3` | `#403A2F` |
| Warm-gray-light | `#E8E4DB` | `#524A3D` |

Alpha ratios in dark mode are slightly higher than light (e.g. 0.75 vs
0.7) because cream-on-ink needs a bit more lift to reach equivalent
perceived contrast.

### Semantic token layer (new)

To support light and dark without conditionals in the consuming code,
introduce a layer of semantic CSS variables defined twice (in `:root`
for light, in `[data-theme="dark"]` for dark):

```css
:root {
  --surface-base:      var(--cream);
  --surface-raised:    var(--cream-light);
  --text-primary:      var(--ink);
  --text-secondary:    var(--ink-70);
  --text-muted:        var(--ink-40);
  --border-strong:     var(--ink);
  --border-subtle:     var(--ink-15);
  --hover-tint:        var(--ink-08);
  --accent-primary:    var(--brick);
  --accent-primary-hover: var(--brick-hover);
  --accent-success:    var(--teal);
  --accent-success-bg: var(--teal-light);
  --accent-warning:    var(--ochre);
  --accent-warning-bg: var(--ochre-light);
}

[data-theme="dark"] {
  --surface-base:      #1A1915;
  --surface-raised:    #221F19;
  --text-primary:      #F5F0E8;
  --text-secondary:    rgba(245,240,232,0.75);
  --text-muted:        rgba(245,240,232,0.5);
  --border-strong:     rgba(245,240,232,0.88);
  --border-subtle:     rgba(245,240,232,0.18);
  --hover-tint:        rgba(245,240,232,0.08);
  --accent-primary:    #E04A38;
  --accent-primary-hover: #CB3E2E;
  --accent-success:    #3FA58F;
  --accent-success-bg: #223A33;
  --accent-warning:    #E8AA2C;
  --accent-warning-bg: #3A2F14;
}
```

Primitive tokens (`--cream`, `--ink`, `--brick`, …) stay unchanged for
backward compatibility and explicit-colour needs. New UI code uses
semantic tokens. Existing `--cream`/`--ink` usages migrate
opportunistically (no big-bang refactor).

### Dark mode toggle

- **Strategy**: `data-theme="dark"` attribute on `<html>`. Tailwind
  configured with `darkMode: ['selector', '[data-theme="dark"]']` so
  `dark:` variants remain available if needed, but the semantic-token
  approach means most code doesn't need them.
- **Resolution order**: `localStorage.theme` > `prefers-color-scheme` >
  light (default).
- **Toggle UI**: one control in the sidebar footer (next to the lang
  switcher). Three-state: Light / Dark / Auto (System). The "Auto"
  option stops writing to localStorage and starts following
  `prefers-color-scheme` live.
- **Flash-of-unstyled-content prevention**: a tiny inline script in
  `index.html` and `workspace.html` `<head>` reads localStorage and
  sets `data-theme` synchronously before CSS paint.

### Grain in dark mode

Reduce opacity from 0.025 to 0.02 — cream noise over dark feels
harsher at full opacity. Same SVG source.

### Typography scale (new)

Replace ad-hoc `text-[Npx]` pixel sizes with a named scale. Implemented
as Tailwind custom utilities in the SPA theme config; equivalent CSS
utilities in `index.html` / `workspace.html`.

| Class | Size / line-height | Font / weight | Transform | Usage |
|---|---|---|---|---|
| `.text-display-xl` | 40px / 1.1 | Bitter 800 | — | Hero headlines |
| `.text-display-lg` | 28px / 1.15 | Bitter 800 | — | Page titles |
| `.text-display` | 20px / 1.2 | Bitter 700 | — | Section / card titles |
| `.text-body` | 15px / 1.6 | DM Sans 500 | — | Paragraph text |
| `.text-body-sm` | 13px / 1.5 | DM Sans 500 | — | Secondary text, user info, nav labels |
| `.text-meta` | 11px / 1.4 | DM Sans 600 | uppercase, tracking-wider | Labels, roles, categories, badges |
| `.text-eyebrow` | 10px / 1.3 | DM Sans 700 | uppercase, tracking-[1.5px] | Section headers inside the sidebar |

All uppercase labels must include `tracking-wider` (meta) or
`tracking-[1.5px]` (eyebrow) — never plain uppercase.

### Borders & radius

- `--border` = `2px solid var(--ink)` — strong containers (cards, modals,
  primary buttons, inputs, sidebar, header).
- `1px` `ink/15` or `ink/10` — internal separators (table rows, calendar
  grid cells).
- `1px` `ink` — badges, mini controls.
- Radius: `3px` everywhere, exposed as Tailwind `rounded-sm`.
  **`rounded-[3px]` in source code is a drift violation.**

### Grain

Keep the existing SVG fractal noise overlay at `opacity: 0.025`, fixed
position, `z-index: 9999`, `pointer-events: none`. Already present in
`index.html` and `workspace.html` — do not re-introduce per-page.

### Spacing

Tailwind default 4px grid. No reinvention.

## 2. Icon system

### Style rules

- SVG only. Black-and-white. **No emojis in UI chrome, ever.**
- `viewBox="0 0 24 24"`, stroke **2px** (matches border weight),
  `stroke="currentColor"`, `fill="none"` (default).
- `stroke-linecap="square"`, `stroke-linejoin="miter"` — no rounded
  caps, no rounded joins. Angular, intentional.
- Integer coordinates only. No `12.5` — keeps rendering crisp at small
  sizes.
- Colour inherited via `currentColor`. Change icon colour by changing
  the parent text colour (`text-ink-70`, `text-brick`, etc.).

### Sizing

| Class | Size | Usage |
|---|---|---|
| `size-4` | 16px | Sidebar nav, card icons, inline |
| `size-5` | 20px | Button icons |
| `size-6` | 24px | Empty states, hero markers |

### Inventory (18 initial icons)

Navigation (11):
- `overview` — grid/dashboard
- `todos` — checkbox
- `notes` — document with lines
- `files` — stacked documents or folder
- `history` — clockwise arrow
- `calendar` — month grid
- `memory` — bookmark or abstract brain (decide during draw)
- `scheduled` — clock
- `settings` — gear
- `workspaces` — grid of squares
- `globe` — for super-admin global observability

Cards & affordances (7):
- `message` — speech bubble
- `recap` — clipboard with lines
- `task-check` — checkmark in square
- `webhook` — chain link
- `bolt` — generic action / zap
- `pin` — thumbtack
- `chevron-down` — workspace selector disclosure

### Integration

Single Leptos component `crates/spa/src/components/icon.rs`:

```rust
#[component]
pub fn Icon(name: &'static str, #[prop(optional)] class: &'static str) -> impl IntoView {
    let svg = match name {
        "calendar" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor"
                 stroke-width="2" stroke-linecap="square" stroke-linejoin="miter">
                <rect x="3" y="5" width="18" height="16"/>
                <path d="M3 9 L21 9 M8 3 L8 7 M16 3 L16 7"/>
            </svg>
        },
        // ... one arm per icon
        _ => view! { <svg viewBox="0 0 24 24"></svg> },
    };
    view! { <span class=class>{svg}</span> }
}
```

Usage: `<Icon name="calendar" class="size-4" />`.

### Source of truth for drawings

- `crates/spa/assets/icons/*.svg` — one hand-edited SVG per icon, the
  authoritative drawing.
- Match arm in `icon.rs` copies the relevant `<path>` from the SVG.
- No build step: edit SVG, paste path, commit both.

### Why not a sprite sheet

- Simpler to inspect in PRs (paths visible in the match).
- Works uniformly with `currentColor` and WASM bundling.
- Revisit if the set grows past ~50 icons.

### Adding a new icon

1. Draw at 24×24, 2px stroke, square caps/joins, integer coords.
2. Save to `crates/spa/assets/icons/<name>.svg`.
3. Add a match arm in `crates/spa/src/components/icon.rs`.
4. Append the name to the inventory in `docs/design-system.md`.

## 3. Component patterns (rules, not templates)

> **Note on tokens used below.** Examples use primitive names
> (`bg-ink`, `text-cream`, `bg-cream-light`) for visual clarity. New
> code should prefer the semantic equivalents (`bg-surface-base`,
> `text-primary`, `bg-surface-raised`, `border-strong`, …) so dark
> mode swaps automatically. The Tailwind theme must expose utility
> classes for each semantic token.

### Borders
- 2px ink — strong containers.
- 1px ink/15 or ink/10 — internal separators.
- 1px ink — badges, mini controls.
- Radius: always `rounded-sm` (= 3px). No `rounded-[3px]`.

### Offset-block shadow
- Signature: `box-shadow: 3px 3px 0 var(--border-strong)`.
  (Uses the semantic token so the shadow inverts correctly in dark
  mode — an ink offset on a dark canvas would disappear.)
- Reserved for hero cards or important framed sections (a prominent KPI
  card, the main modal, a landing section's headline card).
- **Never on list-item cards** (todos, notes, memories in a grid) — it
  becomes visual noise.
- Rule of thumb: 1–2 instances per page max.

### Button
- **Primary**: `bg-ink text-cream border-2 border-ink rounded-sm
  px-4 py-2.5 text-meta font-bold`. Hover: `bg-brick`.
- **Secondary**: `bg-cream-light text-ink border-2 border-ink rounded-sm`
  same dimensions. Hover: `bg-warm-gray-light`.
- **Ghost**: no border, `text-ink-70 hover:text-ink`, underline on hover.
- **Destructive**: `bg-brick text-cream border-2 border-ink`. Reserved
  for deletions.
- All buttons: uppercase + `tracking-wider`. Never `font-normal` or
  lowercase on a button.

### Input
- `border-2 border-ink rounded-sm p-3 bg-cream-light text-body`.
- Focus: `border-brick outline-none`. Never the default browser blue
  ring.

### Card (listed in grid / stack)
- `border-2 border-ink rounded-sm bg-cream-light p-4`.
- No offset-block shadow.
- Title in `text-display` (20px Bitter 700).
- Meta in `text-meta`.

### Nav item (sidebar)
- Base: `text-body-sm text-ink-70 px-5 py-2 border-l-[3px]
  border-transparent hover:bg-ink/[0.04]`.
- Active: `border-l-[3px] border-brick text-ink font-semibold`.
- Icon `size-4` on the left, label `text-body-sm`.

### Badge / Pill
- `border border-ink rounded-sm px-2 py-0.5 text-meta`.
- Status variants: bg `--teal-light` / `--ochre-light` / `--brick` (with
  cream text), border stays 1px ink.

### Empty state
- Upgrade `components/empty_state.rs` from text-only to:
  - Icon `size-6` at the top, colour `--ink-40`.
  - Title in `text-display`.
  - Message in `text-body-sm`, `--ink-40`.
  - Optional secondary CTA button.

### Section header (inside a page)
- Eyebrow (`text-eyebrow`) + title (`text-display-lg`) + optional 2px
  ink underline separator.

## 4. Voice & tone

### Rules
- Short sentences. Implicit subject allowed. Period.
- **No exclamation marks. Ever.** Not in UI, not in i18n strings.
- **No emojis** in UI chrome, nor in i18n strings.
- No "please". Use imperative form.
- No optimistic rephrasing. The action is the result.
- Uppercase for meta / eyebrow labels; sentence case elsewhere.
- Errors: factual and actionable ("Phone number not recognized." — not
  "Oops! We couldn't find that number :(").
- Empty states: brief, sometimes dry. "Nothing yet." is valid. Suggest
  an action only if actually helpful.
- Dates: short, locale-aware. Not "Today at 3:42 PM on Monday".

### Examples

| ✗ | ✓ |
|---|---|
| "Successfully saved your note!" | "Saved." |
| "Oops! Something went wrong." | "Save failed. Retry." |
| "You don't have any todos yet. Create your first one!" | "No todos. Add one from chat or here." |
| "Please enter a valid phone number" | "Invalid phone number." |
| "Welcome back, friend! 👋" | "Hi." |
| "Your workspace has been created 🎉" | "Workspace created." |

### Marketing landing variant (`index.html`)

The landing page may relax the UI-chrome voice **slightly** to carry
enough warmth to convert a visitor who has never heard of Grumps. The
relaxation is narrow:

- Headlines and section intros may be one sentence longer than the
  UI-chrome style would allow.
- Body copy may take a beat to tell what Grumps is and why it matters
  — explanation, not performance.
- Warmth in phrasing is OK ("We listen in your group so you don't have
  to take notes" reads warmer than "Note-free group memory" without
  turning into marketing fluff).

What does **not** relax:
- No exclamation marks. The identity stays dry.
- No emojis.
- No "please" in CTAs. Use imperatives ("Join the waitlist", "See how
  it works").
- No generic SaaS superlatives ("amazing", "game-changing", "powerful").

When in doubt, cut a word. Terseness is the signature.

### LLM prompt alignment
`crates/agent/src/prompt.rs` must inject the same voice rules so bot
replies follow the same tone. Audit item, not part of this spec.

## 5. Audit — current drift

Violations to fix during the alignment implementation.

### Icon drift (emojis to replace)

| File | Emojis | Replacement |
|---|---|---|
| `components/sidebar.rs:74, 89, 90, 91` | 🌐 📅 🧠 ⏰ | `globe`, `calendar`, `memory`, `scheduled` |
| `components/scheduled_card.rs:6-11` | 💬 📋 ✅ 🔗 ⚡ | `message`, `recap`, `task-check`, `webhook`, `bolt` |
| `components/memory_card.rs:89` | 📌 | `pin` |
| `pages/global_observability.rs:278` | 🌐 | `globe` |
| `pages/overview.rs:171` | 📌 | `pin` |

Unicode symbols in the sidebar (`⊫ ☐ ¶ ⟰ ↻ ⚙ ⊞`) also replaced by their
SVG counterparts — not incorrect but inconsistent with the emoji ones
once those are removed.

### Typography drift

Normalize every `text-[Npx]` occurrence to the named scale. Known hits:
`text-[10px]`, `text-[11px]`, `text-[12px]`, `text-[13px]`,
`text-[15px]` across `sidebar.rs`, `lang_switcher.rs`, `memory_card.rs`,
`global_observability.rs`, `calendar/agenda.rs`.

### Border / radius drift
- `rounded-[3px]` → `rounded-sm` (lang_switcher, global_observability).
- Audit `border` vs `border-2` case by case per the section-3 rule.

### Offset-block shadow drift
- Current hits: `global_observability.rs:53, 299` and `login.rs`.
- Decision: keep both — they are hero / modal surfaces. Do not extend
  to other pages.

### Empty state
- `components/empty_state.rs` lacks icon and CTA slot. Upgrade per
  section 3.

### i18n voice audit
- Scan `crates/i18n/locales/en.json` for **every** rule in section 4,
  not just emojis: `!`, emojis, "Successfully"/"Oops"/other
  optimistic rephrasings, "please", superlatives ("amazing",
  "powerful", "game-changing", "seamless"), long-winded sentences
  where a two-word imperative would do.
- Each hit reviewed manually against the section-4 rules.
- Translated locales follow once `en.json` is cleaned.

### Landing voice audit (`index.html`)
- Apply the **full** section 4 + landing-variant rules, not just emoji
  removal: punctuation (`!`), optimistic rephrasing, "please",
  superlatives, empty-slogan filler, overlong sentences.
- Also verify the landing-specific relaxations are used **sparingly**
  — if a section reads like a marketing site, cut it down.
- At t₀ (this audit): quick scan found 0 user-facing `!`, 0 emoji, 0
  superlatives, 0 "please". Landing is already aligned. Record here as
  a snapshot; re-audit before any copy change lands.

### LLM prompt audit
- `crates/agent/src/prompt.rs` — check that voice rules are injected;
  add them if not.

## 6. Persistence & enforcement

### Canonical living document
`docs/design-system.md` — the source of truth. Evolves with the
codebase. This spec (dated under `docs/superpowers/specs/`) captures
decisions and the initial audit at t₀; `design-system.md` is the
document to read before touching UI code.

### CLAUDE.md pointer
Add a short section in `CLAUDE.md`:

```markdown
## Design system

Source of truth: `docs/design-system.md`. All frontend work
(crates/spa/, *.html, i18n strings) must conform to it. Key rules:
- No emoji in UI chrome — use `<Icon name="..."/>`
- Use semantic tokens (`--surface-base`, `--text-primary`, …) for new
  code so it works in both light and dark mode
- Named typography scale (display-xl/lg, display, body, body-sm,
  meta, eyebrow) — no `text-[Npx]` in source
- 2px borders on strong containers; `rounded-sm` (= 3px) everywhere
- Offset-block shadow reserved for hero cards, 1–2 per page max
- Terse voice: no "!", no emoji, no "please". Landing may relax
  slightly but keeps the same rules on punctuation and emojis.
```

### Project-local skill
`.claude/skills/grumps-design/SKILL.md`:

```markdown
---
name: grumps-design
description: Use when touching any UI code in crates/spa/, index.html,
  workspace.html, or i18n strings. Enforces the Grumps warm brutalist
  design system.
---

Read docs/design-system.md before proposing any change to:
- crates/spa/src/**/*.rs
- index.html, workspace.html, workspace.*.html
- crates/i18n/locales/*.json

Validate every change against: icon rules, typography scale, border
and radius rules, offset-block usage, voice & tone.
```

The skill does not duplicate content — it forces a read of the canonical
file. Single source of truth preserved.

### Optional follow-up — lint script
`scripts/check-design-drift.sh` greps the affected files for known
violations (emojis, `text-[Npx]`, `rounded-[*]`, `!` in i18n). Can run
as a pre-commit hook. Out of scope for the initial alignment plan;
noted as a follow-up.

## Implementation plan scope (handed to writing-plans)

An implementation plan executing this spec should:

1. Add Tailwind theme entries + matching CSS utilities for the typo
   scale (in the SPA theme config and in `index.html` / `workspace.html`
   so both surfaces use the same classes).
2. Define the semantic token layer in `:root` and `[data-theme="dark"]`
   across `index.html`, `workspace.html`, and the SPA. Primitives
   stay; semantic tokens are additive.
3. Configure Tailwind `darkMode: ['selector', '[data-theme="dark"]']`.
4. Add the dark-mode toggle control in the sidebar footer (three-state:
   Light / Dark / Auto). Persist to `localStorage.theme`. Inline the
   FOUC-prevention script in `index.html` / `workspace.html` heads.
5. Create `crates/spa/assets/icons/` and draw the 18 SVGs.
6. Create `crates/spa/src/components/icon.rs` with the match table.
7. Wire `mod icon;` and `pub use` in `crates/spa/src/components/mod.rs`.
8. Refactor `sidebar.rs`, `scheduled_card.rs`, `memory_card.rs`,
   `global_observability.rs`, `overview.rs` to use `<Icon>`.
9. Normalize all `text-[Npx]` to the named scale in affected files.
10. Normalize `rounded-[3px]` → `rounded-sm`.
11. Upgrade `empty_state.rs` per section 3.
12. Audit `en.json` for voice violations; fix.
13. Audit `index.html` copy against **all** section-4 rules plus the
    landing variant: `!`, emojis, optimistic rephrasings, "please",
    superlatives, overlong sentences, empty-slogan filler. Keep the
    relaxations sparing — warmth where present, but cut if a section
    reads like a marketing site.
14. Write `docs/design-system.md` (living doc) as a trimmed,
    reader-friendly version of this spec — no audit, no drift history.
15. Add the design-system section to `CLAUDE.md`.
16. Create `.claude/skills/grumps-design/SKILL.md`.

Each step should be small enough to be reviewed independently. The
dark-mode steps (2–4) can ship before the icon refactor (5–8) if
preferred.
