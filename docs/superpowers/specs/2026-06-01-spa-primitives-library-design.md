# In-house accessible UI primitives library + SPA sweep

**Date:** 2026-06-01
**Status:** Approved (design) — pending implementation plan
**Depends on:** the Leptos 0.8 + Tailwind v4 + Switch work (branch `feat/spa-leptos08-rust-ui`, PR #5). This work branches from there (or from `main` once #5 merges).

## Problem

The SPA has one accessible primitive (the rust-ui-derived `Switch`) and a large body of ad-hoc, hand-rolled interactive controls with real accessibility gaps. A full inventory found:

- **6 modals** (memory create/edit + delete-confirm, scheduled create/edit + delete-confirm, dashboard help) — all `fixed inset-0` hand-built with **no `role="dialog"`, no `aria-modal`, no Escape, no focus-trap, no focus-restore, no scroll-lock**.
- **2 dropdown menus** (lang switcher, workspace switcher) — no `aria-haspopup`/`aria-expanded`, no keyboard nav, no `role="menu"`.
- **4 filter/tab rows** (todos, memory, scheduled ×2, calendar view-mode) — `<button>`s with no `role="tablist"`/`aria-selected`/arrow-key nav.
- **40+ ad-hoc buttons** — inline-styled, no shared component, icon-only ones lack `aria-label`.
- **20+ form inputs** — no `<label for/id>` association (only one checkbox is wired correctly); no shared field wrapper.
- **5 native `<select>`** — accessible (native) but unstyled/inconsistent.
- **1 faked `<div>` checkbox** (todo row) — no `role`/keyboard.
- **Toast** — `components/toast.rs` is an empty stub; no reactive queue, never invoked.

rust-ui only cleanly covers a few components: its `Switch` was a clean reactive Leptos component (we adopted it), but its **Dialog and DropdownMenu are inline-`<script>`-JS-blob driven with incomplete a11y** (no focus-trap, no `role`/`aria-modal`/`aria-expanded`, no arrow-key nav) and pull a ~400-line scroll-lock module + icons + their Button. They are not suitable to copy into a reactive CSR Leptos SPA.

## Goals

Build an in-house accessible UI primitives library and convert all SPA controls onto it.

- **Style** stays rust-ui-derived (same Tailwind classes / `data-[state=…]` conventions, consistent with the shipped `Switch`).
- **Behavior** is our own idiomatic reactive Leptos: state in `RwSignal`, conditional render with `<Show>`, DOM event listeners attached via `web-sys` and cleaned up in `on_cleanup`, accessibility baked in.
- The visual design is **unchanged** — this is a component-mechanics + accessibility effort, not a redesign.

## Non-goals

- No visual redesign, no SPA architecture refactor, no new user-facing features beyond the Toast system (which is net-new because the current stub is empty).
- No full custom listbox for `<select>` — see the Select decision below.
- Not touching the worker, landing, or i18n dictionaries (beyond any new keys a converted control needs).

## Decisions (locked with the user)

- **Select → styled native wrapper.** Keep the native `<select>` (already fully accessible + OS keyboard) inside a `Select` component that applies the brutalism styling + a custom chevron. No custom listbox (re-implementing native a11y is risky and costly).
- **Toast → in scope.** Build the reactive toast system (queue + `ToastViewport` + an invocation helper exposed via context), since the current `toast.rs` is an empty stub. This is the one net-new feature.

## The primitive set

All live under a new `crates/spa/src/components/ui/` submodule, re-exported from `components/ui/mod.rs`. `switch.rs` moves into `ui/`. Data-bound cards (`memory_card`, `scheduled_card`, calendar views, etc.) stay in `components/`.

| Primitive | API shape (controlled where stateful) | Accessibility contract |
|---|---|---|
| **Button** | `variant: ButtonVariant` (Primary/Secondary/Danger/Ghost), `size`, `icon_only: bool` + required `aria_label` when icon-only, `disabled: Signal<bool>`, `on_click`, `class` | real `<button>`; `aria-label` enforced for icon-only; `disabled` reflected |
| **Dialog** | `open: RwSignal<bool>`, `on_close`, `title`/`labelledby`, slots for body/footer; `close_on_backdrop: bool` | `role="dialog"` + `aria-modal="true"`, `aria-labelledby`/`aria-describedby`; focus moves in on open, **focus-trap** (Tab cycle), **focus-restore** to trigger on close; Escape closes; backdrop `inert`/`aria-hidden`; body scroll-lock |
| **DropdownMenu** | `open: RwSignal<bool>` (or internal + controlled option), trigger slot, item list | trigger `aria-haspopup="menu"` + `aria-expanded`; panel `role="menu"`, items `role="menuitem"` w/ roving `tabindex`; ArrowUp/Down/Home/End, Escape closes + restores focus, Enter/Space activate; click-outside closes (listener cleaned up) |
| **Tabs / Segmented** | `value: RwSignal<T>` where `T: PartialEq`, tab definitions | `role="tablist"`, each `role="tab"` + `aria-selected`; ArrowLeft/Right (+ Home/End) roving focus |
| **Field** | `label`, `id` (auto if absent), optional `help`/`error`, children = the input | renders `<label for=id>`; wires `aria-describedby` to help/error; consistent label typography |
| **Select** | `value: RwSignal<String>` (or `on_change`), options; wraps native `<select>` | native `<select>` (keeps native a11y) styled + custom chevron |
| **Checkbox** | `checked: Signal<bool>`, `on_change`, `aria_label`/label | native `<input type=checkbox>` styled, or `role="checkbox"` button with Space/Enter |
| **Switch** | already shipped (`checked: Signal<bool>`, `on_change`, reactive `aria_label`) | `role="switch"`, moved into `ui/` |
| **Toast** (net-new) | `ToastViewport` mounted once at app root; a `Toasts` handle in context with `.show(kind, message)`; auto-dismiss timer | `role="status"`/`aria-live="polite"` region; dismissable |

### Conventions

- Class composition via `tw_merge!(base, class)` (the stable crate already added for `Switch`).
- Stateful primitives are **controlled** by a caller-owned signal (the `Switch` pattern), so they integrate with existing page state.
- DOM listeners (Escape, click-outside, focus-trap) use `web-sys` + `wasm-bindgen` closures, registered on mount and removed in `on_cleanup` — no leaked `document` listeners.
- No `leptos_ui`/`clx!` (would pull leptos `nightly`); no inline `<script>` blobs; no portal crate (conditional render + a high-`z-index` overlay).

### New SPA dependency surface

Add `web-sys` features needed for reactive interactivity: `KeyboardEvent`, `MouseEvent`, `EventTarget`, `Element`/`HtmlElement` focus methods, `Event`, plus `wasm-bindgen` `Closure`. (`Switch` needed none of these; Dialog/DropdownMenu/Toast do.) No new crates beyond `web-sys` feature flags.

## Conversion sweep (master list)

Build → convert, primitive by primitive. Targets from the inventory:

- **Button:** all 40+ ad-hoc buttons across `todos.rs`, `memory.rs`, `scheduled.rs`, `settings.rs`, `dashboard.rs`, `note_editor.rs`, `global_settings.rs`, `login.rs`, `memory_card.rs`, `scheduled_card.rs`, `sidebar.rs`. Icon-only (chevrons, "+") get `aria-label`.
- **Field:** form inputs in the memory modal (`memory.rs:220–273`), scheduled modal (`scheduled.rs:244–291`), `note_editor.rs:94–106`, `settings.rs` (iCal URL, display fields), `global_settings.rs:61`.
- **Checkbox:** todo-row faked checkbox (`todos.rs:125–171`); standardize memory pinned (`memory.rs:276–285`).
- **Dialog:** `memory.rs:203–305` + `308–334`; `scheduled.rs:227–308` + `311–337`; `dashboard.rs:107–156`.
- **DropdownMenu:** `lang_switcher.rs:12–49`; `workspace_switcher.rs:6–65`.
- **Tabs/Segmented:** `todos.rs:78–92`; `memory.rs:146–162`; `scheduled.rs:154–188`; `calendar.rs` view-mode selector.
- **Select:** `memory.rs:242–252`; `scheduled.rs:254–264`; `settings.rs:96–117` + `135–143`; `global_settings.rs:66–72`.
- **Toast:** implement `toast.rs` reactive queue + viewport + context handle; mount the viewport at app root; wire at least the obvious success/error spots (e.g. settings save, iCal copy/regenerate/revoke, scheduled/memory create/delete) as the first consumers.

i18n discipline: any new visible string (e.g. icon-button `aria-label`s, toast messages) goes through the i18n layer — add English keys; never hardcode.

## Phasing (sub-PRs, each green and shippable)

Each phase ends green (`cargo test`, SPA `trunk build --release`, visual smoke incl. `ar` RTL + `tr` length) and is its own PR.

- **A — Foundation:** move `Switch` into `ui/`; build `Button`, `Field`, `Checkbox`, `Select` (styled-native); add the `web-sys` features. Convert the high-frequency, low-risk usages: buttons + form fields + selects + the todo checkbox.
- **B — Dialog:** build the accessible `Dialog`; convert all 6 modals.
- **C — DropdownMenu:** build it; convert lang + workspace switchers.
- **D — Tabs/Segmented:** build it; convert the 4 filter/tab rows.
- **E — Toast:** build the reactive toast system; mount the viewport; wire the first consumers.

Phases B–E each depend only on A (and the design tokens), so they can land in order without coupling.

## Verification

Per phase: SPA `trunk build --release` green; `cargo test --target x86_64-pc-windows-msvc` green; `cargo fmt --all --check` green (the rustfmt gate that bit us once); visual smoke of the converted surfaces in default + `ar` (RTL) + `tr` (long strings). For Dialog/DropdownMenu specifically: manual keyboard check (Tab/Shift+Tab trap, Escape, arrow keys) and screen-reader role/aria sanity.

## Risks

- **Focus-trap correctness** is the hardest part — get the Tab/Shift+Tab cycle, initial focus, and focus-restore right; cover the "no focusable children" edge. This is exactly why we build it once, carefully, and reuse it.
- **Listener leaks** — every `document`/`window` listener must be removed in `on_cleanup`; verify no duplicate handlers across open/close cycles.
- **Sweep volume** — 40+ buttons and 20+ fields is mechanical but large; phasing keeps each PR reviewable. A converted control must remain visually identical (no redesign creep).
- **RTL/Tailwind logical props** — Dialog/Dropdown positioning must use logical properties so `ar` mirrors correctly.

## Files (anticipated)

- New: `crates/spa/src/components/ui/{mod,button,dialog,dropdown_menu,tabs,field,select,checkbox,toast}.rs`; `switch.rs` moved into `ui/`.
- Modified: `crates/spa/src/components/mod.rs` (export `ui`), `crates/spa/Cargo.toml` (`web-sys` features), app root (mount `ToastViewport`), and every page/component in the sweep list above.
- i18n: new English keys in `crates/i18n/locales/en.json` for any new aria-labels / toast messages (then the translate pass).
</content>
