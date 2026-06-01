# SPA component-system modernization + workspace-wide dependency bump

**Date:** 2026-06-01
**Status:** Approved (design) — pending implementation plan

## Problem

The SPA is Leptos 0.7 (Rust → WASM) with no component library. Most controls are already native HTML (`<input>`, `<select>`, `<button>` appear 56× across 13 files), which is the right call for this stack. The exception is the settings **toggle**: it is a `<div>` faked into a switch (the `Toggle` component at `crates/spa/src/pages/settings.rs:239`), positioned with magic pixels (`top-[1px]`, `left:2px/18px`). The knob is ~2px too high and horizontally asymmetric — the reported "rond pas centré" bug — and being a `<div>` it has no keyboard focus and no accessibility.

The deeper issue is not "hand-rolled vs library": it is the absence of a disciplined primitive layer. Today we get the worst of both — bespoke controls without the rigor that would make them correct by construction.

A second, initially separate concern: dependencies are stale, and the worker carries a deliberate stop-gap pin (`worker-build@0.8.1` in `wrangler.toml`) added because the latest worker-build demanded `wasm-bindgen >= 0.2.121` while the project pins 0.2.118.

These turn out to be the same problem. The workspace is a single Cargo lockfile, so `wasm-bindgen`/`js-sys`/`web-sys` resolve to one shared version across `grumps-spa` and `grumps-worker`. Migrating the SPA to Leptos 0.8 forces the wasm-bindgen chain up — which is precisely the move that dissolves the worker-build pin. One coordinated bump resolves both.

## Goals

1. Migrate the SPA from Leptos 0.7 to 0.8.
2. Adopt [rust-ui](https://rust-ui.com) (copy-paste registry, Tailwind-based) as the accessible component base; replace the faked toggle with a real, accessible `Switch` re-styled to the design tokens. Establish the pattern for future controls.
3. Bump all workspace dependencies "to the max" in a single coordinated move, lifting the worker-build pin as part of it.

## Non-goals (YAGNI)

- Not rebuilding every control at once. We set up rust-ui, convert the toggle (the reported bug) and anything sharing the same fake-control defect, and leave already-correct native `<select>`/`<input>` controls untouched unless the bump forces a change.
- Not adopting a styled runtime component library (thaw / leptonic) — it would impose its own visual language and fight the bespoke "warm brutalism" design system. Copy-paste was chosen precisely so we own and re-style the code.
- Not changing the worker's runtime behavior, routes, or API. The worker bump is purely dependency versions + lifting the build pin.

## Approach

### Component base: rust-ui (copy-paste)

rust-ui is a shadcn-inspired registry: components are copied into our `components/` directory (via the `ui-cli` crate or manually from the registry), not pulled as a runtime crate dependency. It is Tailwind-based and uses `tailwind_fuse` / `tw_merge!` for class merging.

- The design tokens already live as CSS variables in `crates/spa/input.css` `@layer base` (`--cream`, `--ink`, `--teal`, `--brick`, 2px ink borders, no shadows). Copied components are re-styled to these — they adopt the brutalism rather than impose a foreign look.
- Each rust-ui primitive is wrapped in a thin Leptos component that preserves the current i18n API. The `Toggle` wrapper keeps its `label_key` / `desc_key` / `value` / `on_toggle` signature so the four call sites in `settings.rs` do not churn. The wrapper renders a real accessible `Switch` underneath — centered by construction, with keyboard focus and ARIA for free.
- i18n discipline is maintained: no hardcoded strings; wrappers keep `tr()` keys.

Start with `Switch` (the bug). Pull `Select` / `Button` / `Input` / `Dialog` only as they earn their place.

### Coordinated dependency bump (lockstep on wasm-bindgen)

1. **Workspace root** (`Cargo.toml` `[workspace.dependencies]`): `serde*`, `chrono` 0.4, `chrono-tz` 0.10 → latest, `uuid`, `thiserror` 2 → max.
2. **SPA** (`crates/spa/Cargo.toml`): `leptos` / `leptos_router` 0.7 → 0.8, `gloo-*`, the `wasm-bindgen` / `js-sys` / `web-sys` chain, plus Tailwind v3 → v4 if rust-ui requires it.
3. **Worker** (`crates/worker/Cargo.toml`): `worker` 0.8 → latest with the `worker-build` pin lifted in `wrangler.toml`, `jsonwebtoken` 9, `rusqlite` 0.32, `reqwest` 0.12, crypto crates (`sha2` / `hmac` / `hex`). Other crates (`agent`, `nlu`, `messaging`, `memory`, `scheduler`, `calendar`, `core`): direct deps bumped to max.

The single shared `wasm-bindgen` version means these cannot move independently; the bump is one operation, not three.

### Tailwind v3 → v4

The SPA is currently Tailwind v3 (`@tailwind base/components/utilities` + `tailwind.config.js`). rust-ui likely assumes Tailwind v4 (`@import "tailwindcss"`, CSS-based config). Confirm at rust-ui setup; if required, migrate the SPA (and the landing build, which shares the design system) to v4 syntax. The CSS-variable tokens carry over unchanged.

## Verification (gates before merge)

- `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe test --target x86_64-pc-windows-msvc` — green.
- Worker: `cargo clippy -p grumps-worker --target wasm32-unknown-unknown` + `wrangler deploy --dry-run` — green (proves the lifted pin holds and the worker still builds to wasm).
- SPA: `trunk build --release` — green; demo bundle still builds for the landing iframe.
- Visual: toggle knob centered; RTL `ar` layout intact; `tr` (Turkish, ~15-20% longer) does not overflow.
- CI: a push to `main` runs the existing auto-deploy pipeline green end-to-end (deploy worker + index migrations + workspace migrations + pages).

## Risks & fallbacks

- **Worker pin won't align.** If lifting the pin breaks the wasm-bindgen chain, fall back: the worker stays at 0.8.1 and the SPA still migrates at the highest `wasm-bindgen` version common to both. They are coupled only through that crate.
- **Leptos 0.7 → 0.8 breaking changes.** Signal API and `view!` changes touch every page and component. This is the bulk of the work; budget for a full SPA sweep, not a localized edit.
- **rust-ui Leptos version lag.** If rust-ui does not yet target Leptos 0.8 cleanly, fall back to [RustForWeb/shadcn-ui](https://github.com/RustForWeb/shadcn-ui) or the headless [leptix](https://github.com/leptos-rs/awesome-leptos) (behavior + ARIA, we write all CSS).
- **Tailwind v4 migration scope.** If v4 is required, it touches both the SPA and the landing build. Treat as a bounded sub-task with its own verification (visual diff of landing + SPA across locales).

## Files (anticipated)

- `Cargo.toml` — workspace dep versions bumped.
- `crates/spa/Cargo.toml` — Leptos 0.8, gloo, wasm chain.
- `crates/worker/Cargo.toml` — worker latest, jsonwebtoken/rusqlite/reqwest/crypto.
- `crates/{agent,nlu,messaging,memory,scheduler,calendar,core}/Cargo.toml` — direct deps to max.
- `wrangler.toml` — `worker-build` pin lifted from the `[build]` command (+ comment updated).
- `crates/spa/input.css`, `crates/spa/tailwind.config.js` — Tailwind v4 migration if required.
- `crates/spa/src/components/` — copied rust-ui primitives (Switch first) + thin i18n wrappers.
- `crates/spa/src/pages/settings.rs` — `Toggle` reimplemented on the Switch primitive (call sites unchanged).
- All Leptos 0.7 → 0.8 API touch-ups across `crates/spa/src/`.
</content>
</invoke>
