# Design system alignment — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Formalize the Grumps warm-brutalist design system, add a dark mode that matches the light identity, introduce a custom SVG icon set, and align existing surfaces to the spec.

**Architecture:** A canonical semantic-token layer in CSS custom properties swaps values under `[data-theme="dark"]`; Tailwind exposes utilities for them. A new `Icon` Leptos component holds 18 hand-drawn monochrome SVGs inline (2px stroke, square caps, 24×24 viewBox). Existing files migrate opportunistically — surfaces that render both themes today get migrated now; fine-grained colours migrate when touched. A project-local skill and a CLAUDE.md pointer enforce the design system on future work.

**Tech Stack:** Leptos 0.7 CSR, Tailwind CSS (config at `crates/spa/tailwind.config.js`), Trunk build, static HTML for landing/workspace. Spec: `docs/superpowers/specs/2026-04-20-design-system-design.md`.

---

## File Structure

**Created:**
- `crates/spa/src/components/icon.rs` — `Icon` component, 18-arm match returning inline SVG.
- `crates/spa/src/components/theme_toggle.rs` — three-state Light/Dark/Auto control.
- `crates/spa/assets/icons/*.svg` — 18 hand-edited source SVGs (authoritative drawings).
- `docs/design-system.md` — the living document (trimmed, reader-first).
- `.claude/skills/grumps-design/SKILL.md` — enforcement skill.

**Modified:**
- `crates/spa/input.css` — add semantic token layer, dark palette, typography utilities, dark-mode grain.
- `crates/spa/tailwind.config.js` — add `darkMode` selector, semantic colour utilities, typography scale.
- `crates/spa/index.html` — add FOUC-prevention inline script.
- `crates/spa/src/components/mod.rs` — export `Icon` and `ThemeToggle`.
- `crates/spa/src/components/sidebar.rs` — replace emojis and Unicode symbols with `<Icon>`, add `<ThemeToggle>`, migrate colour styles to semantic tokens.
- `crates/spa/src/components/scheduled_card.rs` — replace emoji map with icon names.
- `crates/spa/src/components/memory_card.rs` — replace 📌 with `pin` icon.
- `crates/spa/src/components/empty_state.rs` — accept `icon` prop, optional CTA.
- `crates/spa/src/pages/global_observability.rs` — replace 🌐 with `globe` icon.
- `crates/spa/src/pages/overview.rs` — replace 📌 with `pin` icon.
- `crates/spa/src/**/*.rs` — normalize `text-[Npx]` → named classes; `rounded-[3px]` → `rounded-sm`.
- `index.html` (marketing landing) — add semantic token layer + dark palette; copy audit against landing variant.
- `workspace.html` (static mockup) — add semantic token layer + dark palette.
- `crates/i18n/locales/en.json` — voice-violation audit.
- `CLAUDE.md` — add design-system section.

---

## Task 1: HTML token layer — SPA stylesheet

**Files:**
- Modify: `crates/spa/input.css`

Adds semantic tokens (layer over existing primitives), the dark palette under `[data-theme="dark"]`, typography utilities, and dark-mode grain opacity override. Existing primitives and grain stay.

- [ ] **Step 1: Extend `input.css`**

Replace the current `@layer base` block with:

```css
@layer base {
  :root {
    /* Primitives (unchanged) */
    --cream: #F5F0E8;
    --cream-light: #FAF8F4;
    --ink: #1A1915;
    --ink-70: rgba(26, 25, 21, 0.7);
    --ink-40: rgba(26, 25, 21, 0.4);
    --ink-15: rgba(26, 25, 21, 0.15);
    --ink-08: rgba(26, 25, 21, 0.08);
    --brick: #C0392B;
    --brick-hover: #A93226;
    --teal: #1B6B5A;
    --teal-light: #E8F5F0;
    --ochre: #D4940A;
    --ochre-light: #FFF8E7;
    --warm-gray: #D5CFC3;
    --warm-gray-light: #E8E4DB;
    --font-display: 'Bitter', Georgia, serif;
    --font-body: 'DM Sans', -apple-system, sans-serif;

    /* Semantic layer (light defaults) */
    --surface-base:         var(--cream);
    --surface-raised:       var(--cream-light);
    --text-primary:         var(--ink);
    --text-secondary:       var(--ink-70);
    --text-muted:           var(--ink-40);
    --border-strong:        var(--ink);
    --border-subtle:        var(--ink-15);
    --hover-tint:           var(--ink-08);
    --accent-primary:       var(--brick);
    --accent-primary-hover: var(--brick-hover);
    --accent-success:       var(--teal);
    --accent-success-bg:    var(--teal-light);
    --accent-warning:       var(--ochre);
    --accent-warning-bg:    var(--ochre-light);
    --grain-opacity:        0.025;
  }

  [data-theme="dark"] {
    --surface-base:         #1A1915;
    --surface-raised:       #221F19;
    --text-primary:         #F5F0E8;
    --text-secondary:       rgba(245, 240, 232, 0.75);
    --text-muted:           rgba(245, 240, 232, 0.5);
    --border-strong:        rgba(245, 240, 232, 0.88);
    --border-subtle:        rgba(245, 240, 232, 0.18);
    --hover-tint:           rgba(245, 240, 232, 0.08);
    --accent-primary:       #E04A38;
    --accent-primary-hover: #CB3E2E;
    --accent-success:       #3FA58F;
    --accent-success-bg:    #223A33;
    --accent-warning:       #E8AA2C;
    --accent-warning-bg:    #3A2F14;
    --grain-opacity:        0.02;
  }

  body {
    font-family: var(--font-body);
    font-size: 14.5px;
    line-height: 1.55;
    color: var(--text-primary);
    background: var(--surface-base);
    -webkit-font-smoothing: antialiased;
  }

  body::before {
    content: '';
    position: fixed;
    inset: 0;
    z-index: 9999;
    pointer-events: none;
    opacity: var(--grain-opacity);
    background-image: url("data:image/svg+xml,%3Csvg viewBox='0 0 256 256' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='noise'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='4' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23noise)'/%3E%3C/svg%3E");
    background-size: 256px 256px;
  }

  * { box-sizing: border-box; margin: 0; padding: 0; }

  ::-webkit-scrollbar { width: 6px; }
  ::-webkit-scrollbar-track { background: transparent; }
  ::-webkit-scrollbar-thumb { background: var(--border-subtle); border-radius: 3px; }
  ::-webkit-scrollbar-thumb:hover { background: var(--text-muted); }
}

@layer utilities {
  /* Typography scale — canonical named sizes */
  .text-display-xl  { font-family: var(--font-display); font-weight: 800; font-size: 40px; line-height: 1.1;  letter-spacing: -0.01em; }
  .text-display-lg  { font-family: var(--font-display); font-weight: 800; font-size: 28px; line-height: 1.15; letter-spacing: -0.005em; }
  .text-display     { font-family: var(--font-display); font-weight: 700; font-size: 20px; line-height: 1.2; }
  .text-body        { font-family: var(--font-body); font-weight: 500; font-size: 15px; line-height: 1.6; }
  .text-body-sm     { font-family: var(--font-body); font-weight: 500; font-size: 13px; line-height: 1.5; }
  .text-meta        { font-family: var(--font-body); font-weight: 600; font-size: 11px; line-height: 1.4; text-transform: uppercase; letter-spacing: 0.05em; }
  .text-eyebrow     { font-family: var(--font-body); font-weight: 700; font-size: 10px; line-height: 1.3; text-transform: uppercase; letter-spacing: 1.5px; }
}
```

- [ ] **Step 2: Build CSS and verify**

Run: `cd crates/spa && npx tailwindcss -i input.css -o dist/styles.css --minify 2>&1 | tail -10`
Expected: no errors; output file generated.

Alternatively with Trunk: `cd crates/spa && trunk build 2>&1 | tail -20`
Expected: build succeeds.

- [ ] **Step 3: Verify CSS contains new tokens**

Run: `grep -o 'surface-base\|text-primary\|accent-primary' crates/spa/dist/styles.css | sort -u`
Expected: all three tokens appear.

- [ ] **Step 4: Commit**

```bash
git add crates/spa/input.css
git commit -m "style(spa): semantic token layer + dark palette + typography utilities"
```

---

## Task 2: HTML token layer — landing + workspace mockup

**Files:**
- Modify: `index.html` (`:root` block around line 18–39)
- Modify: `workspace.html` (same pattern)

- [ ] **Step 1: Update `index.html` tokens**

Find the `:root { ... }` block (around line 18). Append the semantic token and dark variants inside/after it so landing can also switch themes. Replace the block with:

```css
:root {
  --cream:        #F5F0E8;
  --cream-light:  #FAF8F4;
  --ink:          #1A1915;
  --ink-70:       rgba(26, 25, 21, 0.7);
  --ink-40:       rgba(26, 25, 21, 0.4);
  --ink-15:       rgba(26, 25, 21, 0.15);
  --ink-08:       rgba(26, 25, 21, 0.08);
  --brick:        #C0392B;
  --brick-hover:  #A93226;
  --teal:         #1B6B5A;
  --teal-light:   #E8F5F0;
  --ochre:        #D4940A;
  --ochre-light:  #FFF8E7;
  --warm-gray:    #D5CFC3;
  --warm-gray-light: #E8E4DB;

  --font-display: 'Bitter', Georgia, serif;
  --font-body:    'DM Sans', -apple-system, sans-serif;
  --border:       2px solid var(--border-strong);
  --radius:       3px;

  /* Semantic (light defaults) */
  --surface-base:         var(--cream);
  --surface-raised:       var(--cream-light);
  --text-primary:         var(--ink);
  --text-secondary:       var(--ink-70);
  --text-muted:           var(--ink-40);
  --border-strong:        var(--ink);
  --border-subtle:        var(--ink-15);
  --hover-tint:           var(--ink-08);
  --accent-primary:       var(--brick);
  --accent-primary-hover: var(--brick-hover);
  --accent-success:       var(--teal);
  --accent-success-bg:    var(--teal-light);
  --accent-warning:       var(--ochre);
  --accent-warning-bg:    var(--ochre-light);
  --grain-opacity:        0.025;
}

[data-theme="dark"] {
  --surface-base:         #1A1915;
  --surface-raised:       #221F19;
  --text-primary:         #F5F0E8;
  --text-secondary:       rgba(245, 240, 232, 0.75);
  --text-muted:           rgba(245, 240, 232, 0.5);
  --border-strong:        rgba(245, 240, 232, 0.88);
  --border-subtle:        rgba(245, 240, 232, 0.18);
  --hover-tint:           rgba(245, 240, 232, 0.08);
  --accent-primary:       #E04A38;
  --accent-primary-hover: #CB3E2E;
  --accent-success:       #3FA58F;
  --accent-success-bg:    #223A33;
  --accent-warning:       #E8AA2C;
  --accent-warning-bg:    #3A2F14;
  --grain-opacity:        0.02;
}
```

Then find the `body { color: var(--ink); background: var(--cream); ... }` and the `body::before { ... opacity: 0.025; ... }` rules and change them to use semantic tokens:
```css
body { ... color: var(--text-primary); background: var(--surface-base); ... }
body::before { ... opacity: var(--grain-opacity); ... }
```

- [ ] **Step 2: Apply the same changes to `workspace.html`**

Identical replacement for the `:root { ... }` block and the body rules.

- [ ] **Step 3: Smoke-test both files in a browser**

Open `index.html` directly. Expected: page renders identically to before. Open dev tools and set `document.documentElement.setAttribute('data-theme', 'dark')`. Expected: page shifts to dark mode (dark warm bg, cream text); no broken surfaces in the hero or first screenful.

Repeat for `workspace.html`.

- [ ] **Step 4: Commit**

```bash
git add index.html workspace.html
git commit -m "style: semantic token layer + dark palette on landing and workspace mockup"
```

---

## Task 3: Tailwind config — darkMode + semantic utilities + typography scale

**Files:**
- Modify: `crates/spa/tailwind.config.js`

- [ ] **Step 1: Replace config**

```js
/** @type {import('tailwindcss').Config} */
module.exports = {
  darkMode: ['selector', '[data-theme="dark"]'],
  content: [
    "./index.html",
    "./src/**/*.rs",
  ],
  theme: {
    extend: {
      colors: {
        // Primitives (kept for explicit uses)
        cream: { DEFAULT: '#F5F0E8', light: '#FAF8F4' },
        ink: { DEFAULT: '#1A1915' },
        brick: { DEFAULT: '#C0392B', hover: '#A93226' },
        teal: { DEFAULT: '#1B6B5A', light: '#E8F5F0' },
        ochre: { DEFAULT: '#D4940A', light: '#FFF8E7' },
        'warm-gray': { DEFAULT: '#D5CFC3', light: '#E8E4DB' },

        // Semantic tokens — flat keys so Tailwind generates clean
        // utility names (bg-surface, text-primary, border-strong, …).
        // CSS variables resolve the correct value per theme.
        surface: {
          DEFAULT: 'var(--surface-base)',
          raised: 'var(--surface-raised)',
        },
        primary:   'var(--text-primary)',
        secondary: 'var(--text-secondary)',
        muted:     'var(--text-muted)',
        strong:    'var(--border-strong)',
        subtle:    'var(--border-subtle)',
        'hover-tint': 'var(--hover-tint)',
        accent: {
          DEFAULT: 'var(--accent-primary)',
          hover:   'var(--accent-primary-hover)',
        },
        success: {
          DEFAULT: 'var(--accent-success)',
          bg:      'var(--accent-success-bg)',
        },
        warning: {
          DEFAULT: 'var(--accent-warning)',
          bg:      'var(--accent-warning-bg)',
        },
      },
      fontFamily: {
        display: ['Bitter', 'Georgia', 'serif'],
        body: ['DM Sans', '-apple-system', 'sans-serif'],
      },
      borderWidth: {
        'grumps': '2px',
      },
    },
  },
  plugins: [],
}
```

Note: the typography utilities (`.text-display-xl` etc.) are defined in `input.css` `@layer utilities` (Task 1), not via Tailwind's `fontSize`. This keeps font-family, weight, size, and letter-spacing bundled as one class.

- [ ] **Step 2: Rebuild and verify class generation**

Run: `cd crates/spa && npx tailwindcss -i input.css -o dist/styles.css --minify 2>&1 | tail -10`
Expected: no errors.

- [ ] **Step 3: Verify semantic utilities compile**

Run: `grep -oE 'bg-surface|text-primary|border-strong' crates/spa/dist/styles.css | sort -u | head -5`
Expected: at least one match — but Tailwind only emits utilities for classes it sees in content. If empty, that's fine until a component uses them (Task 5+ will).

- [ ] **Step 4: Commit**

```bash
git add crates/spa/tailwind.config.js
git commit -m "build(spa): tailwind darkMode selector + semantic colour utilities"
```

---

## Task 4: FOUC-prevention inline script

**Files:**
- Modify: `crates/spa/index.html`
- Modify: `index.html` (landing)
- Modify: `workspace.html`

Prevents a flash of light content when the user's preference is dark, by reading `localStorage.theme` synchronously before first paint.

- [ ] **Step 1: Add script to `crates/spa/index.html` `<head>`**

Insert just before `</head>`:

```html
<script>
  (function () {
    try {
      var t = localStorage.getItem('theme');
      var auto = (t === null || t === 'auto');
      var dark = auto
        ? window.matchMedia('(prefers-color-scheme: dark)').matches
        : t === 'dark';
      if (dark) document.documentElement.setAttribute('data-theme', 'dark');
    } catch (e) {}
  })();
</script>
```

- [ ] **Step 2: Add identical script to `index.html` (landing) `<head>`**

Insert just before `</head>`.

- [ ] **Step 3: Add identical script to `workspace.html` `<head>`**

Insert just before `</head>`.

- [ ] **Step 4: Verify in browser**

Run: `cd crates/spa && trunk serve` (or open the static files directly).
In the browser console: `localStorage.setItem('theme', 'dark'); location.reload();`
Expected: page loads in dark mode with no visible flash of light content.
Then: `localStorage.setItem('theme', 'light'); location.reload();`
Expected: page loads in light mode.

- [ ] **Step 5: Commit**

```bash
git add crates/spa/index.html index.html workspace.html
git commit -m "feat(theme): FOUC-prevention script reads theme before first paint"
```

---

## Task 5: Icon component with all 18 icons

**Files:**
- Create: `crates/spa/src/components/icon.rs`
- Modify: `crates/spa/src/components/mod.rs`

- [ ] **Step 1: Create `crates/spa/src/components/icon.rs`**

```rust
use leptos::prelude::*;

/// Monochrome 2px-stroke SVG icon. Inherits colour via `currentColor`.
/// Sizing is done by the parent (Tailwind `size-4` / `size-5` / `size-6`).
///
/// Source drawings live in `crates/spa/assets/icons/<name>.svg`.
/// When adding an icon, paste its `<path>` content into the match below
/// and keep the viewBox at `0 0 24 24`, stroke-width `2`, square caps.
#[component]
pub fn Icon(
    name: &'static str,
    #[prop(optional, default = "")] class: &'static str,
) -> impl IntoView {
    let svg = match name {
        "overview" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <rect x="4" y="4" width="7" height="7"/>
                <rect x="13" y="4" width="7" height="7"/>
                <rect x="4" y="13" width="7" height="7"/>
                <rect x="13" y="13" width="7" height="7"/>
            </svg>
        }.into_any(),
        "todos" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <rect x="4" y="4" width="16" height="16"/>
                <path d="M8 12 L11 15 L16 9"/>
            </svg>
        }.into_any(),
        "notes" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <path d="M5 3 L15 3 L19 7 L19 21 L5 21 Z"/>
                <path d="M15 3 L15 7 L19 7"/>
                <path d="M8 11 H16 M8 15 H16 M8 19 H13"/>
            </svg>
        }.into_any(),
        "files" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <path d="M3 7 L3 19 L21 19 L21 9 L11 9 L9 7 Z"/>
            </svg>
        }.into_any(),
        "history" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <path d="M4 12 A8 8 0 1 1 8 18"/>
                <path d="M4 8 L4 14 L10 14"/>
                <path d="M12 8 L12 12 L15 14"/>
            </svg>
        }.into_any(),
        "calendar" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <rect x="3" y="5" width="18" height="16"/>
                <path d="M3 9 L21 9 M8 3 L8 7 M16 3 L16 7"/>
            </svg>
        }.into_any(),
        "memory" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <path d="M6 3 L18 3 L18 21 L12 16 L6 21 Z"/>
            </svg>
        }.into_any(),
        "scheduled" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <circle cx="12" cy="12" r="8"/>
                <path d="M12 7 L12 12 L16 14"/>
            </svg>
        }.into_any(),
        "settings" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <circle cx="12" cy="12" r="3"/>
                <path d="M12 2 L12 5 M12 19 L12 22 M22 12 L19 12 M5 12 L2 12 M19 5 L17 7 M7 17 L5 19 M19 19 L17 17 M7 7 L5 5"/>
            </svg>
        }.into_any(),
        "workspaces" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <rect x="3" y="3" width="18" height="18"/>
                <path d="M12 3 L12 21 M3 12 L21 12"/>
            </svg>
        }.into_any(),
        "globe" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <circle cx="12" cy="12" r="9"/>
                <path d="M3 12 L21 12"/>
                <ellipse cx="12" cy="12" rx="4" ry="9"/>
            </svg>
        }.into_any(),
        "message" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <path d="M3 5 L21 5 L21 17 L11 17 L7 21 L7 17 L3 17 Z"/>
            </svg>
        }.into_any(),
        "recap" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <rect x="5" y="5" width="14" height="16"/>
                <rect x="8" y="3" width="8" height="4"/>
                <path d="M9 11 H15 M9 15 H15 M9 19 H13"/>
            </svg>
        }.into_any(),
        "task-check" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <circle cx="12" cy="12" r="9"/>
                <path d="M7 12 L11 16 L17 9"/>
            </svg>
        }.into_any(),
        "webhook" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <path d="M4 13 L4 15 A2 2 0 0 0 6 17 L10 17 A2 2 0 0 0 12 15 L12 9 A2 2 0 0 1 14 7 L18 7 A2 2 0 0 1 20 9 L20 11"/>
            </svg>
        }.into_any(),
        "bolt" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <path d="M13 3 L5 14 L11 14 L9 21 L19 10 L13 10 Z"/>
            </svg>
        }.into_any(),
        "pin" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <path d="M8 4 L16 4 L14 10 L18 14 L6 14 L10 10 Z"/>
                <path d="M12 14 L12 21"/>
            </svg>
        }.into_any(),
        "chevron-down" => view! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="100%" height="100%">
                <path d="M5 9 L12 16 L19 9"/>
            </svg>
        }.into_any(),
        _ => view! {
            <svg viewBox="0 0 24 24"></svg>
        }.into_any(),
    };
    view! { <span class=class>{svg}</span> }
}
```

- [ ] **Step 2: Export from `components/mod.rs`**

Add to `crates/spa/src/components/mod.rs`:
```rust
pub mod icon;
pub use icon::Icon;
```

- [ ] **Step 3: Build and verify**

Run: `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe check -p grumps-spa --target wasm32-unknown-unknown 2>&1 | tail -20`
Expected: no errors. (If the SPA crate uses a different target, follow the project's existing build command.)

- [ ] **Step 4: Smoke-test in the browser**

Temporarily drop `<Icon name="calendar" class="size-6 text-accent"/>` somewhere visible (e.g. in `pages/overview.rs`), run `trunk serve`, and confirm the icon renders as an angular black-and-white calendar glyph. Revert the temporary insertion before commit.

- [ ] **Step 5: Commit**

```bash
git add crates/spa/src/components/icon.rs crates/spa/src/components/mod.rs
git commit -m "feat(spa): Icon component with 18 warm-brutalist SVGs"
```

---

## Task 6: Save SVG source files

**Files:**
- Create: `crates/spa/assets/icons/*.svg` (18 files)

Each SVG holds exactly the body the component renders, in a file named `<name>.svg`. These are the authoritative drawings; the component match is a transcription.

- [ ] **Step 1: Create the directory**

```bash
mkdir -p crates/spa/assets/icons
```

- [ ] **Step 2: Write each SVG file**

Each file is a complete standalone SVG, e.g. `crates/spa/assets/icons/calendar.svg`:

```xml
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" stroke-linejoin="miter" width="24" height="24">
  <rect x="3" y="5" width="18" height="16"/>
  <path d="M3 9 L21 9 M8 3 L8 7 M16 3 L16 7"/>
</svg>
```

Repeat for each of the 18 icons. The `<rect>` / `<path>` bodies are the same as in Task 5's match arms. The 18 names (matching Task 5):
`overview.svg`, `todos.svg`, `notes.svg`, `files.svg`, `history.svg`, `calendar.svg`, `memory.svg`, `scheduled.svg`, `settings.svg`, `workspaces.svg`, `globe.svg`, `message.svg`, `recap.svg`, `task-check.svg`, `webhook.svg`, `bolt.svg`, `pin.svg`, `chevron-down.svg`.

- [ ] **Step 3: Verify all 18 files exist**

Run: `ls crates/spa/assets/icons/ | wc -l`
Expected: `18`.

- [ ] **Step 4: Open a few in a browser to visually verify they match the component**

Open `crates/spa/assets/icons/calendar.svg`, `scheduled.svg`, `memory.svg` as files in the browser. Expected: each renders as a black line drawing matching its name.

- [ ] **Step 5: Commit**

```bash
git add crates/spa/assets/icons/
git commit -m "feat(spa): SVG source files for the 18-icon set"
```

---

## Task 7: Migrate sidebar emojis and Unicode symbols to `<Icon>`

**Files:**
- Modify: `crates/spa/src/components/sidebar.rs`

- [ ] **Step 1: Replace emoji spans and Unicode NavItem icons**

Open `crates/spa/src/components/sidebar.rs`. Change the imports at the top to include `Icon`:

```rust
use leptos::prelude::*;
use leptos_router::components::A;
use crate::auth::use_auth;
use crate::i18n::tr;
use crate::components::lang_switcher::LangSwitcher;
use crate::components::Icon;
```

Replace the super-admin emoji block (the `<span class="w-[18px] ...">"🌐"</span>` inside the super admin link) with:
```rust
<Icon name="globe" class="size-4 flex-shrink-0"/>
```

Replace the NavItem icon argument: change its signature so it takes an `icon_name: &'static str` that it passes to `<Icon>` instead of a Unicode character. Full replacement of the `NavItem` component and the nav list:

```rust
// Inside Sidebar, replace the NavItem usages
<NavItem href=base.clone()                       key="sidebar.nav.overview"  icon="overview"    />
<NavItem href=format!("{}/todos",     base)      key="sidebar.nav.todos"     icon="todos"       />
<NavItem href=format!("{}/notes",     base)      key="sidebar.nav.notes"     icon="notes"       />
<NavItem href=format!("{}/files",     base)      key="sidebar.nav.files"     icon="files"       />
<NavItem href=format!("{}/history",   base)      key="sidebar.nav.history"   icon="history"     />
<NavItem href=format!("{}/calendar",  base)      key="sidebar.nav.calendar"  icon="calendar"    />
<NavItem href=format!("{}/memory",    base)      key="sidebar.nav.memory"    icon="memory"      />
<NavItem href=format!("{}/scheduled", base)      key="sidebar.nav.scheduled" icon="scheduled"   />

<div class="px-5 pt-4 pb-1.5 text-eyebrow text-muted">
    {move || tr("sidebar.section.manage")}
</div>
<NavItem href=format!("{}/settings",  base)      key="sidebar.nav.settings"  icon="settings"    />

<A href=format!("{}/dashboard", prefix)
   attr:class="flex items-center gap-2.5 px-5 py-2 text-body-sm font-medium cursor-pointer transition-all border-l-[3px] border-transparent hover:bg-hover-tint text-secondary">
    <Icon name="workspaces" class="size-4 flex-shrink-0"/>
    {move || tr("sidebar.my_workspaces")}
</A>
```

Replace the `NavItem` definition at the bottom of the file:

```rust
#[component]
fn NavItem(href: String, key: &'static str, icon: &'static str) -> impl IntoView {
    view! {
        <A href=href
           attr:class="flex items-center gap-2.5 px-5 py-2 text-body-sm font-medium cursor-pointer transition-all border-l-[3px] border-transparent hover:bg-hover-tint text-secondary">
            <Icon name=icon class="size-4 flex-shrink-0"/>
            {move || tr(key)}
        </A>
    }
}
```

Also replace the eyebrow `<div class="px-5 pt-4 pb-1.5 text-[10px] font-bold uppercase tracking-[1.5px]" style="color: var(--ink-40);">` usages (workspace + manage headers) with `<div class="px-5 pt-4 pb-1.5 text-eyebrow text-muted">`, and the super-admin eyebrow with `text-eyebrow text-accent`.

Remove all `style="color: var(--ink-70);"`-style inline colour on nav items — Tailwind classes handle it.

- [ ] **Step 2: Build and run type-check**

Run: `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe check -p grumps-spa --target wasm32-unknown-unknown 2>&1 | tail -20`
Expected: no errors.

- [ ] **Step 3: Visual check**

`cd crates/spa && trunk serve`, open the workspace sidebar. Expected: all nav items show monochrome angular icons; active item has brick left-border; hovering shows a subtle hover tint.

- [ ] **Step 4: Grep for residual emojis/Unicode icons in this file**

Run: `grep -nP '[\x{1F300}-\x{1FAFF}\x{2600}-\x{27BF}]|\\u\{[0-9A-F]{4,5}\}' crates/spa/src/components/sidebar.rs`
Expected: no matches.

- [ ] **Step 5: Commit**

```bash
git add crates/spa/src/components/sidebar.rs
git commit -m "refactor(sidebar): migrate emojis and Unicode icons to Icon component"
```

---

## Task 8: Migrate remaining emoji usages

**Files:**
- Modify: `crates/spa/src/components/scheduled_card.rs`
- Modify: `crates/spa/src/components/memory_card.rs`
- Modify: `crates/spa/src/pages/global_observability.rs`
- Modify: `crates/spa/src/pages/overview.rs`

- [ ] **Step 1: `scheduled_card.rs` — replace emoji helper**

The current helper returns emojis by kind. Replace the function and its caller with an icon-name lookup and a rendered `<Icon>`:

```rust
// before: fn type_emoji(kind: &str) -> &'static str { match kind { "message" => "💬", ... } }
// after:
fn type_icon(kind: &str) -> &'static str {
    match kind {
        "message"    => "message",
        "recap"      => "recap",
        "task"       => "task-check",
        "webhook"    => "webhook",
        _            => "bolt",
    }
}
```

At the call site, swap the emoji string render for `<Icon name=type_icon(kind) class="size-4 text-muted"/>`. Add `use crate::components::Icon;` at the top.

- [ ] **Step 2: `memory_card.rs` — replace 📌**

Add `use crate::components::Icon;` at top. Replace line 89's `{if pinned { "📌 Pinned" } else { "Pin" }}` with:
```rust
{if pinned {
    view! { <span class="inline-flex items-center gap-1"><Icon name="pin" class="size-3"/>{move || tr("memory.pinned")}</span> }.into_any()
} else {
    view! { <span>{move || tr("memory.pin")}</span> }.into_any()
}}
```
And ensure the i18n keys `memory.pinned` / `memory.pin` exist (add with values `"Pinned"` / `"Pin"` in `crates/i18n/locales/en.json` if missing — translations follow the i18n workflow).

- [ ] **Step 3: `global_observability.rs` — replace 🌐**

Add `use crate::components::Icon;`. Replace line 278 `<span class="w-[18px] text-center text-[15px]">"🌐"</span>` with:
```rust
<Icon name="globe" class="size-4 flex-shrink-0"/>
```

- [ ] **Step 4: `overview.rs` — replace 📌 on line 171**

Add `use crate::components::Icon;`. Replace `"📌 "` in the heading with an inline icon:
```rust
<h3 class="text-display mb-3 flex items-center gap-2">
    <Icon name="pin" class="size-4 text-muted"/>
    {move || tr("overview.what_i_know")}
</h3>
```

- [ ] **Step 5: Verify no emojis left in SPA source**

Run: `grep -rnP '[\x{1F300}-\x{1FAFF}\x{2600}-\x{27BF}]' crates/spa/src/`
Expected: no matches (or only in comments — if any, move them to a note and remove).

- [ ] **Step 6: Build**

Run: `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe check -p grumps-spa --target wasm32-unknown-unknown 2>&1 | tail -20`
Expected: no errors.

- [ ] **Step 7: Commit**

```bash
git add crates/spa/src/components/scheduled_card.rs crates/spa/src/components/memory_card.rs crates/spa/src/pages/global_observability.rs crates/spa/src/pages/overview.rs crates/i18n/locales/en.json
git commit -m "refactor(spa): replace remaining emoji usages with Icon component"
```

---

## Task 9: Normalize typography — `text-[Npx]` → named classes

**Files:**
- Modify: various under `crates/spa/src/` (see grep below)

- [ ] **Step 1: Inventory current violations**

Run: `grep -rnE 'text-\[[0-9]+px\]' crates/spa/src/`
Note the full list. Typical mappings:
- `text-[10px]` (uppercase, tracking-[1.5px]) → `text-eyebrow` (remove the tracking/uppercase classes too, they're folded in)
- `text-[11px]` (uppercase tracking-wider) → `text-meta`
- `text-[11px]` (not uppercase) → `text-body-sm` (rare)
- `text-[12px]` → `text-body-sm`
- `text-[13px]` → `text-body-sm`
- `text-[15px]` → `text-body`
- `text-xl font-display font-extrabold` → `text-display-lg` (or `text-display` if 20px)

- [ ] **Step 2: Apply replacements**

For each hit, substitute the named class and remove now-redundant classes (`uppercase`, `tracking-*`, `font-bold`, `font-semibold`, explicit `font-display`/`font-body`) that the named class already folds in. Keep weight classes only when they differ from the scale default — e.g. a nav-active item wants `font-semibold` on top of `text-body-sm`, that's fine.

Example change in `sidebar.rs`:
```rust
// before
<h1 class="font-display text-xl font-extrabold uppercase tracking-tight">
// after
<h1 class="text-display-lg uppercase tracking-tight">
```

- [ ] **Step 3: Verify no arbitrary pixel text sizes remain**

Run: `grep -rnE 'text-\[[0-9]+px\]' crates/spa/src/`
Expected: no matches.

- [ ] **Step 4: Build**

Run: `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe check -p grumps-spa --target wasm32-unknown-unknown 2>&1 | tail -20`
Expected: no errors.

- [ ] **Step 5: Visual check**

`trunk serve`, walk through: overview, todos, notes, files, calendar, memory, scheduled, settings pages. Expected: typography is cohesive — no visually jarring size jumps; headings read as display, meta labels read as uppercase small caps.

- [ ] **Step 6: Commit**

```bash
git add crates/spa/src/
git commit -m "refactor(spa): normalize typography to named scale (display/body/meta/eyebrow)"
```

---

## Task 10: Normalize radius — `rounded-[3px]` → `rounded-sm`

**Files:**
- Modify: `crates/spa/src/components/lang_switcher.rs`
- Modify: `crates/spa/src/pages/global_observability.rs`
- Any others surfaced by grep

- [ ] **Step 1: Find hits**

Run: `grep -rnE 'rounded-\[3px\]' crates/spa/src/`

- [ ] **Step 2: Replace with `rounded-sm`**

Simple text substitution in each file. The Tailwind default `rounded-sm` equals 2px; if the project needs exactly 3px everywhere, add to `tailwind.config.js` under `theme.extend.borderRadius`:
```js
borderRadius: { sm: '3px' },
```
Do this once during this task if the grep shows pervasive `rounded-[3px]` usage.

- [ ] **Step 3: Verify**

Run: `grep -rnE 'rounded-\[3px\]' crates/spa/src/`
Expected: no matches.

- [ ] **Step 4: Build + visual check**

Run: `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe check -p grumps-spa --target wasm32-unknown-unknown 2>&1 | tail -20`
`trunk serve`, confirm cards and buttons still have subtle rounded corners.

- [ ] **Step 5: Commit**

```bash
git add crates/spa/src/ crates/spa/tailwind.config.js
git commit -m "refactor(spa): normalize border radius to rounded-sm (3px via theme override)"
```

---

## Task 11: Upgrade `empty_state.rs` — icon + optional CTA

**Files:**
- Modify: `crates/spa/src/components/empty_state.rs`

- [ ] **Step 1: Extend the component**

Replace the file contents with:

```rust
use leptos::prelude::*;
use crate::components::Icon;

#[component]
pub fn EmptyState(
    title: String,
    message: String,
    #[prop(optional, default = "")] icon: &'static str,
    #[prop(optional)] cta: Option<(String, String)>,
) -> impl IntoView {
    view! {
        <div class="text-center py-16 max-w-sm mx-auto">
            {(!icon.is_empty()).then(|| view! {
                <div class="inline-flex items-center justify-center text-muted mb-4">
                    <Icon name=icon class="size-6"/>
                </div>
            })}
            <h3 class="text-display">{title}</h3>
            <p class="text-body-sm mt-2 text-muted">{message}</p>
            {cta.map(|(label, href)| view! {
                <a href=href
                   class="inline-flex items-center gap-2 mt-6 px-4 py-2.5 text-meta border-2 border-strong rounded-sm bg-surface-raised text-primary hover:bg-hover-tint transition-colors">
                    {label}
                </a>
            })}
        </div>
    }
}
```

- [ ] **Step 2: Update every existing call site**

Run: `grep -rn 'EmptyState' crates/spa/src/`
For each hit, optionally add an `icon="..."` prop matching the page's domain (todos → `"todos"`, notes → `"notes"`, files → `"files"`, etc.). Existing callers compile unchanged because `icon` is optional.

- [ ] **Step 3: Build**

Run: `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe check -p grumps-spa --target wasm32-unknown-unknown 2>&1 | tail -20`
Expected: no errors.

- [ ] **Step 4: Visual check**

`trunk serve`, navigate to a page with an empty list (e.g. a fresh workspace with no todos). Expected: empty state shows an icon above the title, subdued muted colour.

- [ ] **Step 5: Commit**

```bash
git add crates/spa/src/components/empty_state.rs crates/spa/src/pages/
git commit -m "feat(spa): EmptyState supports icon and optional CTA"
```

---

## Task 12: Theme toggle component

**Files:**
- Create: `crates/spa/src/components/theme_toggle.rs`
- Modify: `crates/spa/src/components/mod.rs`
- Modify: `crates/spa/src/components/sidebar.rs`

- [ ] **Step 1: Create `crates/spa/src/components/theme_toggle.rs`**

```rust
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::window;
use crate::i18n::tr;

/// Three-state theme toggle: Light / Dark / Auto.
/// Persists to `localStorage.theme`; "auto" removes the key and follows
/// `prefers-color-scheme`. Writes `data-theme="dark"` on `<html>` when dark.
#[component]
pub fn ThemeToggle() -> impl IntoView {
    let initial = current_mode();
    let (mode, set_mode) = signal(initial);

    Effect::new(move |_| {
        let m = mode.get();
        apply_mode(&m);
    });

    let set = move |m: &'static str| {
        let _ = move || m; // capture
        set_mode.set(m.to_string());
    };

    view! {
        <div class="inline-flex border-grumps border-strong rounded-sm overflow-hidden"
             role="group"
             aria-label=move || tr("theme.toggle_label")>
            <button
                type="button"
                class=move || btn_class(&mode.get(), "light")
                on:click=move |_| set("light")>
                {move || tr("theme.light")}
            </button>
            <button
                type="button"
                class=move || btn_class(&mode.get(), "auto")
                on:click=move |_| set("auto")>
                {move || tr("theme.auto")}
            </button>
            <button
                type="button"
                class=move || btn_class(&mode.get(), "dark")
                on:click=move |_| set("dark")>
                {move || tr("theme.dark")}
            </button>
        </div>
    }
}

fn btn_class(current: &str, mine: &str) -> String {
    let active = current == mine;
    let base = "px-2.5 py-1 text-meta cursor-pointer transition-colors";
    if active {
        // Active = inverted pair that works in both themes:
        // light → dark bg / cream text; dark → cream bg / dark text.
        format!("{base} bg-primary text-surface")
    } else {
        format!("{base} text-secondary hover:bg-hover-tint")
    }
}

fn current_mode() -> String {
    let Some(win) = window() else { return "auto".into(); };
    let Ok(Some(storage)) = win.local_storage() else { return "auto".into(); };
    match storage.get_item("theme").ok().flatten() {
        Some(v) if v == "light" || v == "dark" || v == "auto" => v,
        _ => "auto".into(),
    }
}

fn apply_mode(mode: &str) {
    let Some(win) = window() else { return; };
    let Some(doc) = win.document() else { return; };
    let Some(root) = doc.document_element() else { return; };
    let resolve_dark = |m: &str| -> bool {
        match m {
            "dark" => true,
            "light" => false,
            _ => win
                .match_media("(prefers-color-scheme: dark)")
                .ok()
                .flatten()
                .map(|mql| mql.matches())
                .unwrap_or(false),
        }
    };
    let dark = resolve_dark(mode);
    if dark {
        let _ = root.set_attribute("data-theme", "dark");
    } else {
        let _ = root.remove_attribute("data-theme");
    }
    if let Ok(Some(storage)) = win.local_storage() {
        if mode == "auto" {
            let _ = storage.remove_item("theme");
        } else {
            let _ = storage.set_item("theme", mode);
        }
    }
}
```

Add i18n keys to `crates/i18n/locales/en.json`:
- `"theme.light": "Light"`
- `"theme.dark": "Dark"`
- `"theme.auto": "Auto"`
- `"theme.toggle_label": "Theme"`

- [ ] **Step 2: Export**

Add to `crates/spa/src/components/mod.rs`:
```rust
pub mod theme_toggle;
pub use theme_toggle::ThemeToggle;
```

- [ ] **Step 3: Wire into sidebar footer**

In `crates/spa/src/components/sidebar.rs`, next to the lang switcher footer row, add a new row:

```rust
<div class="px-5 py-3 border-t border-subtle flex items-center justify-between gap-2">
    <span class="text-eyebrow text-muted">{move || tr("theme.toggle_label")}</span>
    <ThemeToggle />
</div>
```

Add `use crate::components::ThemeToggle;` at the top.

- [ ] **Step 4: Build**

Run: `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe check -p grumps-spa --target wasm32-unknown-unknown 2>&1 | tail -20`
Expected: no errors. (If `web_sys` features are missing, add them to `crates/spa/Cargo.toml`'s `web-sys` feature list: `Window`, `Document`, `Element`, `Storage`, `MediaQueryList`.)

- [ ] **Step 5: Manual test**

`trunk serve`. Click Light / Auto / Dark in turn. Expected:
- Each click flips the theme on the page.
- Reloading preserves the selection.
- In Auto mode, reloading after changing OS theme follows the new OS theme.

- [ ] **Step 6: Commit**

```bash
git add crates/spa/src/components/theme_toggle.rs crates/spa/src/components/mod.rs crates/spa/src/components/sidebar.rs crates/i18n/locales/en.json crates/spa/Cargo.toml
git commit -m "feat(spa): three-state theme toggle (Light/Auto/Dark) in sidebar"
```

---

## Task 13: Migrate top-level surfaces to semantic tokens

Goal: the pages you look at daily render cleanly in both themes. Deeper detail colour usages migrate later when those components are touched.

**Files:**
- Modify: `crates/spa/src/components/sidebar.rs`
- Modify: `crates/spa/src/components/header.rs`
- Modify: `crates/spa/src/pages/global_observability.rs`
- Modify: `crates/spa/src/pages/login.rs`

- [ ] **Step 1: Sidebar**

In `sidebar.rs`, replace inline style/colour references:
- `style="background: var(--cream-light);"` → remove inline style, add class `bg-surface-raised`.
- `border-r-2 border-ink` → `border-r-2 border-strong`.
- `border-b-2 border-ink` inside the brand header → `border-b-2 border-strong`.
- Any `style="color: var(--ink-40);"` → class `text-muted`.
- Any `style="color: var(--ink-70);"` → class `text-secondary`.
- `<span class="... text-brick">` → `text-accent` (where the colour should also adapt to dark).

- [ ] **Step 2: Header**

In `header.rs`, do the same substitutions: background/surface classes, border/text colour classes.

- [ ] **Step 3: Global observability**

In `global_observability.rs`, same substitutions. Pay special attention to the offset-block card around line 53 / 299: change `box-shadow: 3px 3px 0 #1A1A1A;` (or `var(--ink)`) to `box-shadow: 3px 3px 0 var(--border-strong);`. Same for any `box-shadow: 3px 3px 0 ...` in other files — grep them all.

Run: `grep -rn 'box-shadow.*3px 3px 0' crates/spa/src/ index.html workspace.html`
For each hit, swap the colour to `var(--border-strong)`.

- [ ] **Step 4: Login page**

In `login.rs`, same substitutions. The card uses `border-2 border-ink` → keep the class but it already resolves via the primitive; migrate the inline `style="background: var(--cream-light);"` to `bg-surface-raised`. Error box (`border-brick text-brick`) → `border-accent text-accent`.

- [ ] **Step 5: Build**

Run: `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe check -p grumps-spa --target wasm32-unknown-unknown 2>&1 | tail -20`
Expected: no errors.

- [ ] **Step 6: Visual check in both themes**

`trunk serve`. Walk through sidebar, dashboard/overview, global observability, login, in both light and dark. Expected: surfaces are consistent within each theme; no element stays "stuck in light mode" on the primary surfaces.

- [ ] **Step 7: Commit**

```bash
git add crates/spa/src/
git commit -m "refactor(spa): migrate top-level surfaces to semantic theme tokens"
```

---

## Task 14: i18n voice audit — `en.json`

**Files:**
- Modify: `crates/i18n/locales/en.json`

- [ ] **Step 1: Scan for violations**

Run each scan and record hits:
```bash
grep -nE '!"' crates/i18n/locales/en.json                       # exclamation marks
grep -niE '"please ' crates/i18n/locales/en.json                # "please"
grep -niE 'successfully|oops|whoops|hooray|awesome|amazing|powerful|seamless|game.?chang|revolutionary|incredible' crates/i18n/locales/en.json
grep -nP '[\x{1F300}-\x{1FAFF}\x{2600}-\x{27BF}]|:\)|:\(' crates/i18n/locales/en.json  # emojis & emoticons
```

- [ ] **Step 2: Fix each hit**

Replacement guidelines (from spec §4 Voice & tone):
- `"Successfully saved!"` → `"Saved."`
- `"Oops! Something went wrong."` → `"Something failed. Retry."`
- `"Please enter a valid phone number"` → `"Invalid phone number."`
- Emojis / emoticons → delete.
- Any exclamation → period.

Each string reviewed in isolation — keep the meaning, cut the performance.

- [ ] **Step 3: Re-scan**

Repeat the greps from Step 1. Expected: no matches.

- [ ] **Step 4: Sanity-check JSON validity**

Run: `node -e "JSON.parse(require('fs').readFileSync('crates/i18n/locales/en.json', 'utf8')); console.log('ok')"`
Expected: `ok`.

- [ ] **Step 5: Commit**

```bash
git add crates/i18n/locales/en.json
git commit -m "chore(i18n): align en.json copy with terse warm-brutalist voice"
```

Note: other locales re-translate via the existing i18n workflow (out of scope — their strings are translations of now-updated English, scheduled separately).

---

## Task 15: Landing voice audit — `index.html`

**Files:**
- Modify: `index.html` (as needed)

- [ ] **Step 1: Scan**

Run:
```bash
grep -nE '[a-zA-Z]!' index.html                                   # exclamations in copy (not comments)
grep -niE 'please |amazing|powerful|seamless|game.?chang|incredible|revolutionary|cutting.?edge' index.html
grep -nP '[\x{1F300}-\x{1FAFF}\x{2600}-\x{27BF}]' index.html
```
Also read the file section by section and flag any sentence that reads like a marketing site (generic superlatives, forced excitement, cliché phrasing).

- [ ] **Step 2: Fix hits**

Apply section-4 rules plus landing variant:
- Delete `!`s.
- Remove "please" from CTAs — use imperatives.
- Replace SaaS superlatives with specific concrete phrases.
- Trim any section that reads overlong; two-sentence paragraph is usually enough.
- Keep warmth that earns its place (narrative explanation of Grumps, one sentence longer than UI-chrome is OK).

As of the t₀ snapshot recorded in the spec, the landing already passes. If Step 1 is empty and nothing reads off, no edit is needed — the task is a no-op commit-skip.

- [ ] **Step 3: Re-scan**

Repeat greps. Expected: no matches.

- [ ] **Step 4: Commit (if changes)**

```bash
git add index.html
git commit -m "copy(landing): align with terse voice rules"
```

---

## Task 16: Living document — `docs/design-system.md`

**Files:**
- Create: `docs/design-system.md`

Reader-first trimmed version of the spec. No audit, no drift history — that lives in the spec under `docs/superpowers/specs/`. This doc is what a new contributor reads when touching UI.

- [ ] **Step 1: Write the file**

```markdown
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
`text-secondary`, `text-muted`, `border-strong`, `border-subtle`, `bg-accent`,
`bg-hover-tint`, etc.

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
```

- [ ] **Step 2: Commit**

```bash
git add docs/design-system.md
git commit -m "docs: canonical living document for the design system"
```

---

## Task 17: CLAUDE.md pointer

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Add a Design-system section after the current Design-system block**

The existing `## Design system` section in `CLAUDE.md` describes the aesthetic. Replace that section with:

```markdown
## Design system

Source of truth: `docs/design-system.md`. All frontend work
(`crates/spa/`, `*.html`, i18n strings) must conform to it.
Key rules:
- No emoji in UI chrome — use `<Icon name="..."/>`.
- Use semantic tokens (`--surface-base`, `--text-primary`, …) in new
  code so it works in both light and dark mode.
- Named typography scale (`display-xl/lg`, `display`, `body`,
  `body-sm`, `meta`, `eyebrow`) — no `text-[Npx]` in source.
- 2px borders on strong containers; `rounded-sm` (= 3px) everywhere.
- Offset-block shadow reserved for hero cards, 1–2 per page max.
- Terse voice: no "!", no emoji, no "please". The landing may relax
  slightly for warmth but keeps the punctuation and emoji rules.
```

(The current rules about warm brutalism, colours, typography, voice
are now implicit — captured in the living doc.)

- [ ] **Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs(claude-md): point to docs/design-system.md as the canonical reference"
```

---

## Task 18: Project-local skill

**Files:**
- Create: `.claude/skills/grumps-design/SKILL.md`

- [ ] **Step 1: Create skill file**

```markdown
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
```

- [ ] **Step 2: Verify structure**

Run: `cat .claude/skills/grumps-design/SKILL.md | head -5`
Expected: the frontmatter is valid.

- [ ] **Step 3: Commit**

```bash
git add .claude/skills/grumps-design/SKILL.md
git commit -m "chore(skills): add grumps-design skill to enforce the design system"
```

---

## Final verification

After all tasks land:

- [ ] **V1: Full SPA build**

Run: `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe check -p grumps-spa --target wasm32-unknown-unknown`
Expected: no errors.

- [ ] **V2: Workspace tests**

Run: `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe test --target x86_64-pc-windows-msvc`
Expected: tests that previously passed still pass. (The icon/typography refactor should not break unit tests.)

- [ ] **V3: No residual drift**

```bash
grep -rnP '[\x{1F300}-\x{1FAFF}\x{2600}-\x{27BF}]' crates/spa/src/ index.html workspace.html
grep -rnE 'text-\[[0-9]+px\]|rounded-\[[0-9]+px\]' crates/spa/src/
grep -nE '!"' crates/i18n/locales/en.json
```
Expected: no matches.

- [ ] **V4: Dark mode smoke test**

`trunk serve` the SPA. Toggle through Light / Auto / Dark. Walk the
top-level pages (overview, todos, notes, files, calendar, memory,
scheduled, settings, global-observability). Expected: legible in both
themes; no surface stays locked in the wrong theme.

- [ ] **V5: Icons render in all affected components**

Visit sidebar, scheduled cards, memory cards, global observability,
overview. Expected: each previously-emoji slot now shows a monochrome
angular icon that inherits the surrounding text colour.
