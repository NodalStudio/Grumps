---
name: grumps-design
description: Use when touching any UI code in crates/spa/, index.html, workspace.html, or i18n strings. Enforces the Grumps warm brutalist design system.
---

# Grumps design enforcement

Before making any change to these paths:
- `crates/spa/src/**/*.rs`
- `index.html`, `workspace.html`, `workspace.*.html`
- `crates/i18n/locales/*.json`

Read the canonical reference: **`docs/design-system.md`**.

Validate every proposed change against:

1. **Icons** — no emojis or Unicode glyphs in UI; use
   `<Icon name="..."/>` with a name from the inventory.
2. **Semantic tokens** — new code uses `--surface-*`, `--text-*`,
   `--border-*`, `--accent-*` so dark mode swaps automatically.
3. **Typography** — named classes only (`text-display*`, `text-body*`,
   `text-meta`, `text-eyebrow`). No `text-[Npx]`.
4. **Borders & radius** — 2px on containers, 1px on separators,
   `rounded-sm` everywhere. No `rounded-[Npx]`.
5. **Offset-block shadow** — `box-shadow: 3px 3px 0 var(--border-strong)`,
   hero/modal only, 1–2 per page.
6. **Voice** — no "!", no emoji, no "please". Imperative form.
   Landing allows slightly longer sentences but keeps the same
   punctuation and emoji rules.

If a proposed change violates any rule, adjust it before writing or
flag it and ask.
