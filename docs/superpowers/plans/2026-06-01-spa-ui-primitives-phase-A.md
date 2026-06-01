# UI Primitives Library — Phase A (Foundation) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up the `components/ui/` primitives submodule and ship the foundation primitives — Button, Field, Checkbox, Select (styled-native) — moving the existing Switch into it, then convert the SPA's ad-hoc buttons, form fields, selects, and the faked todo checkbox onto them. Visual appearance unchanged; accessibility improved.

**Architecture:** Idiomatic reactive Leptos 0.8 components styled with the Tailwind v4 design-token utilities (`bg-ink`, `text-cream`, `border-ink`, `text-brick`, etc., exposed via `@theme inline`). Class overrides merged with `tw_merge!`. Controlled where stateful (the Switch pattern). Phase A primitives are static/markup-level (no new `web-sys` listeners — those arrive with Dialog/Dropdown in Phase B/C).

**Tech Stack:** Leptos 0.8 CSR + Trunk, Tailwind v4, `tw_merge` 0.1.21.

**Branch:** `feat/spa-ui-primitives` (already created, off `feat/spa-leptos08-rust-ui` since this depends on the Switch + Tailwind v4 work in PR #5).

**Design tokens available as utilities** (from `crates/spa/input.css` `@theme inline`): `bg-cream`, `bg-ink`, `bg-teal`, `bg-brick`, `text-cream`, `text-ink`, `text-teal`, `text-brick`, `border-ink`, `border-brick`, `border-teal`, `font-display`, `font-body`. Alpha tokens (`--ink-08`, `--ink-40`, …) remain available via inline `style="…var(--ink-40)…"`.

**Commit convention:** gitmoji, bypassing commitizen — `SKIP=commitizen git commit --no-verify -m "<emoji> <subject>"`. No `Co-Authored-By`; no plan identifiers.

**Gate commands** (used throughout; use the pinned cargo per CLAUDE.md, not the Chocolatey shim):
- Build: `cd crates/spa && PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe build --target wasm32-unknown-unknown`
- Format (the gate that bit us once): `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe fmt --all --check`
- Release build: `cd crates/spa && MSYS_NO_PATHCONV=1 trunk build --release`

---

## Task A.1: Create the `ui/` submodule and move Switch into it

**Files:**
- Create: `crates/spa/src/components/ui/mod.rs`
- Move: `crates/spa/src/components/switch.rs` → `crates/spa/src/components/ui/switch.rs`
- Modify: `crates/spa/src/components/mod.rs`
- Modify: `crates/spa/src/pages/settings.rs` (import path)

- [ ] **Step 1: Move the file**

```bash
git mv crates/spa/src/components/switch.rs crates/spa/src/components/ui/switch.rs
```

- [ ] **Step 2: Create `crates/spa/src/components/ui/mod.rs`**

```rust
//! Accessible UI primitives. Styled with the Tailwind v4 design tokens,
//! behavior implemented in idiomatic reactive Leptos (controlled by caller
//! signals). No `leptos_ui`/`nightly`, no inline-script blobs.

pub mod switch;
```

- [ ] **Step 3: In `crates/spa/src/components/mod.rs`, replace the `pub mod switch;` line with `pub mod ui;`** (keep all other `pub mod` lines as-is).

- [ ] **Step 4: Update the import in `crates/spa/src/pages/settings.rs`**

Change `use crate::components::switch::Switch;` to:
```rust
use crate::components::ui::switch::Switch;
```

- [ ] **Step 5: Build + fmt**

Run the build gate, then the fmt gate. Expected: both PASS.

- [ ] **Step 6: Commit**

```bash
SKIP=commitizen git commit --no-verify -m "♻️ Move Switch into new components/ui primitives submodule"
```

---

## Task A.2: Build the `Button` primitive

**Files:**
- Create: `crates/spa/src/components/ui/button.rs`
- Modify: `crates/spa/src/components/ui/mod.rs`

- [ ] **Step 1: Write `crates/spa/src/components/ui/button.rs`**

```rust
use leptos::prelude::*;
use tw_merge::tw_merge;

/// Visual variant. Maps the existing ad-hoc button styles:
/// Primary = filled ink; Secondary = ink outline; Danger = brick outline;
/// Ghost = no border.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    Danger,
    Ghost,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonSize {
    #[default]
    Default,
    Sm,
}

impl ButtonVariant {
    fn classes(self) -> &'static str {
        match self {
            ButtonVariant::Primary => "bg-ink text-cream border-2 border-ink",
            ButtonVariant::Secondary => "bg-transparent text-ink border-2 border-ink",
            ButtonVariant::Danger => "bg-transparent text-brick border border-brick",
            ButtonVariant::Ghost => "bg-transparent text-ink border-2 border-transparent",
        }
    }
}

impl ButtonSize {
    fn classes(self, icon_only: bool) -> &'static str {
        match (self, icon_only) {
            (ButtonSize::Default, false) => "px-4 py-2 text-sm",
            (ButtonSize::Default, true) => "p-2",
            (ButtonSize::Sm, false) => "px-3 py-1.5 text-xs",
            (ButtonSize::Sm, true) => "p-1.5",
        }
    }
}

/// Accessible button. A real `<button>`; icon-only buttons MUST pass
/// `aria_label`. `disabled` is reactive.
#[component]
pub fn Button(
    #[prop(optional)] variant: ButtonVariant,
    #[prop(optional)] size: ButtonSize,
    /// Icon-only buttons get square padding and require `aria_label`.
    #[prop(optional)] icon_only: bool,
    /// Required for icon-only buttons; optional otherwise.
    #[prop(into, optional)] aria_label: MaybeProp<String>,
    #[prop(into, optional)] disabled: Signal<bool>,
    /// Fired on activation (click / Space / Enter via the native button).
    on_click: impl Fn() + 'static,
    #[prop(into, optional, default = String::new())] class: String,
    children: Children,
) -> impl IntoView {
    let merged = tw_merge!(
        "inline-flex items-center justify-center gap-1.5 font-bold rounded-xs \
         cursor-pointer transition-colors disabled:opacity-40 disabled:cursor-not-allowed \
         focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-teal",
        variant.classes(),
        size.classes(icon_only),
        class
    );
    view! {
        <button
            type="button"
            class=merged
            aria-label=move || aria_label.get()
            disabled=move || disabled.get()
            on:click=move |_| on_click()
        >
            {children()}
        </button>
    }
}
```

- [ ] **Step 2: Export it — add `pub mod button;` to `crates/spa/src/components/ui/mod.rs`.**

- [ ] **Step 3: Build + fmt** (gates). Expected PASS (dead-code warning on `Button` until first use is acceptable).

- [ ] **Step 4: Commit**

```bash
SKIP=commitizen git commit --no-verify -m "✨ Add accessible Button primitive (variants/sizes/icon-only)"
```

---

## Task A.3: Convert ad-hoc buttons to `Button`

Mechanical sweep. For each site: replace the raw `<button class=… style=…>` with `<Button variant=… size=… on_click=… >`, preserving the exact visual (pick the variant/size whose `classes()` match the old inline style; pass any leftover layout class like `shrink-0`/`w-full`/`mt-1` via `class=`). Icon-only buttons (chevrons, "+") MUST get `aria_label=` from an i18n key. Handlers that were `on:click=move |_| body` become `on_click=move || body`; a named `Fn(MouseEvent)` handler is wrapped as `on_click=move || handler_logic` (inline its body, or adapt the closure to take no arg).

**Files (each modified; add `use crate::components::ui::button::{Button, ButtonVariant, ButtonSize};` where used):**
- `crates/spa/src/pages/settings.rs` (save_agent → Primary; iCal copy/regenerate → Secondary Sm; iCal revoke → Danger Sm)
- `crates/spa/src/pages/todos.rs` (add-todo header → Primary, icon "+")
- `crates/spa/src/pages/memory.rs` (add-memory header → Primary; modal save → Primary, cancel → Secondary; delete-confirm → Danger)
- `crates/spa/src/pages/scheduled.rs` (add header → Primary; modal save/cancel; delete-confirm → Danger)
- `crates/spa/src/pages/dashboard.rs` (help open → Secondary/Ghost; help close → Primary)
- `crates/spa/src/pages/note_editor.rs` (edit/preview toggle → Secondary; save → Primary, with `disabled`)
- `crates/spa/src/pages/global_settings.rs` (save → Primary)
- `crates/spa/src/components/memory_card.rs` (edit → Secondary Sm; pin → Secondary/Primary Sm; delete → Danger Sm; all icon-ish → `aria_label`)
- `crates/spa/src/components/scheduled_card.rs` (edit → Secondary Sm; delete → Danger Sm)
- `crates/spa/src/components/lang_switcher.rs`, `crates/spa/src/components/workspace_switcher.rs` (trigger chevron buttons → icon-only `Button` with `aria_label`; leave the open/close logic alone — full dropdown conversion is Phase C)
- `crates/spa/src/pages/login.rs` (WhatsApp/Discord stubs → Secondary, `disabled=true`)

Leave `<a>` links as `<a>` (not buttons). Leave the filter-chip `<button>`s alone (Phase D converts them to Tabs).

- [ ] **Step 1: Worked example — primary (settings save), from `settings.rs:161-165`**

Before:
```rust
<button
    class="px-4 py-2 text-sm font-bold border-2 border-ink rounded-xs cursor-pointer"
    style="background: var(--ink); color: var(--cream);"
    on:click=save_agent
>{move || tr("settings.save_agent")}</button>
```
After:
```rust
<Button variant=ButtonVariant::Primary on_click=move || save_agent(())>
    {move || tr("settings.save_agent")}
</Button>
```
(If `save_agent` is a closure taking an event, either change its definition to `move |_: ()|`/no-arg, or inline: `on_click=move || { /* save_agent body */ }`. Pick the smaller diff.)

- [ ] **Step 2: Worked example — danger Sm (iCal revoke), from `settings.rs:191-195`**

After:
```rust
<Button variant=ButtonVariant::Danger size=ButtonSize::Sm
    on_click=move || set_ical_url.set(String::new())>
    {move || tr("settings.ical.revoke")}
</Button>
```

- [ ] **Step 3: Worked example — icon-only "+" (todos add), preserving the label**

If the "+" button had visible text it stays as children; if it's a bare "+" glyph, mark `icon_only=true` and supply `aria_label=tr("…add…")` (reuse the existing action's i18n key; do NOT hardcode English).

- [ ] **Step 4: Apply across all files in the list above.** After each file, run the build gate to keep errors localized.

- [ ] **Step 5: i18n for any new aria-labels** — if an icon-only button needs an `aria_label` and no suitable key exists, add an English key to `crates/i18n/locales/en.json` (e.g. `"todos.add": "Add todo"`) and reference it via `tr(...)`. Never hardcode a visible/assistive string.

- [ ] **Step 6: Build + fmt + release build** (all three gates). Expected PASS.

- [ ] **Step 7: Visual smoke** — `trunk serve`; verify the converted buttons look identical to before in default, `?lang=ar` (RTL), `?lang=tr` (long labels don't clip). Note any visual drift and fix via the `class=` passthrough.

- [ ] **Step 8: Commit**

```bash
SKIP=commitizen git commit --no-verify -m "♻️ Convert ad-hoc buttons to the Button primitive"
```

---

## Task A.4: Build `Field` and `Checkbox` primitives

**Files:**
- Create: `crates/spa/src/components/ui/field.rs`
- Create: `crates/spa/src/components/ui/checkbox.rs`
- Modify: `crates/spa/src/components/ui/mod.rs`

- [ ] **Step 1: Write `crates/spa/src/components/ui/field.rs`**

A label+control wrapper that wires `<label for>` ↔ control `id` and optional help/error text via `aria-describedby`. It does not own the input; the caller passes the control as children and uses the provided `id`.

```rust
use leptos::prelude::*;

/// Form field wrapper: renders a `<label>` bound to the control via `id`,
/// plus optional help/error text wired through `aria-describedby`.
/// The caller renders the actual input as children and sets its `id=id`
/// and `aria-describedby=describedby` (both provided via the render-prop
/// closure) so association is correct.
#[component]
pub fn Field(
    /// Localized label text.
    #[prop(into)] label: String,
    /// Stable id for the control; also used for `for`/`aria-describedby`.
    #[prop(into)] id: String,
    /// Optional localized help text shown under the control.
    #[prop(into, optional)] help: MaybeProp<String>,
    children: Children,
) -> impl IntoView {
    let help_id = format!("{}-help", id);
    let has_help = help.get().is_some();
    view! {
        <div class="flex flex-col gap-1.5 py-2">
            <label for=id.clone() class="text-[11px] font-bold uppercase tracking-wider"
                style="color: var(--ink-40);">
                {label}
            </label>
            {children()}
            <Show when=move || has_help>
                <p id=help_id.clone() class="text-xs" style="color: var(--ink-40);">
                    {move || help.get().unwrap_or_default()}
                </p>
            </Show>
        </div>
    }
}
```
Note: the control inside must carry `id=<same id>`; the convention is the caller passes the same id string to both `Field` and the input. (`aria-describedby` wiring to `{id}-help` is set by the caller on the input when help is present; the plan's conversion task does this.)

- [ ] **Step 2: Write `crates/spa/src/components/ui/checkbox.rs`**

```rust
use leptos::prelude::*;
use tw_merge::tw_merge;

/// Accessible checkbox — a real `<input type="checkbox">` styled to the
/// design system. Controlled by the caller's signal.
#[component]
pub fn Checkbox(
    #[prop(into)] checked: Signal<bool>,
    on_change: impl Fn() + 'static,
    /// Used as the input id and the `for` of an optional adjacent label.
    #[prop(into)] id: String,
    #[prop(into, optional)] aria_label: MaybeProp<String>,
    #[prop(into, optional, default = String::new())] class: String,
) -> impl IntoView {
    let merged = tw_merge!(
        "size-4 border-2 border-ink rounded-xs cursor-pointer accent-teal \
         focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-teal",
        class
    );
    view! {
        <input
            type="checkbox"
            id=id
            class=merged
            aria-label=move || aria_label.get()
            prop:checked=move || checked.get()
            on:change=move |_| on_change()
        />
    }
}
```

- [ ] **Step 3: Export both — add `pub mod field;` and `pub mod checkbox;` to `ui/mod.rs`.**

- [ ] **Step 4: Build + fmt** (gates). Expected PASS.

- [ ] **Step 5: Commit**

```bash
SKIP=commitizen git commit --no-verify -m "✨ Add Field wrapper and accessible Checkbox primitives"
```

---

## Task A.5: Convert form fields and the faked todo checkbox

**Files (modify; add the relevant `use crate::components::ui::{field::Field, checkbox::Checkbox};`):**
- `crates/spa/src/pages/memory.rs` (modal text/textarea/date inputs `220-273` wrapped in `Field` with proper ids; pinned checkbox `276-285` → `Checkbox` keeping its label)
- `crates/spa/src/pages/scheduled.rs` (modal inputs `244-291` → `Field`)
- `crates/spa/src/pages/note_editor.rs` (`94-106` title/content → `Field`)
- `crates/spa/src/pages/global_settings.rs` (`61` display name → `Field`)
- `crates/spa/src/pages/settings.rs` (iCal URL readonly → `Field` with the copy button beside it)
- `crates/spa/src/pages/todos.rs` (the faked `<div>` checkbox at `125-171` → `Checkbox` with `aria_label`)

- [ ] **Step 1: Worked example — a text Field (memory key)**

Before (current shape): a `<label class="text-[11px]…">` followed by an `<input class="border-2 border-ink rounded-xs …">` with no `for`/`id`.
After:
```rust
<Field label=tr("memory.field.key") id="mem-key".to_string()>
    <input
        type="text"
        id="mem-key"
        class="border-2 border-ink rounded-xs px-3 py-2 text-sm bg-transparent outline-hidden"
        prop:value=key
        on:input=move |ev| set_key.set(event_target_value(&ev))
    />
</Field>
```
Keep the input's existing classes/handlers verbatim; only add `id=` matching the `Field` id and wrap it. Use stable, unique ids per modal (e.g. `mem-key`, `mem-value`, `mem-expires`, `sched-title`, …).

- [ ] **Step 2: Worked example — the todo checkbox**

Replace the clickable `<div class="todo-checkbox" on:click=…>` with:
```rust
<Checkbox
    checked=Signal::derive(move || todo.done)
    on_change=move || toggle_done()
    id=format!("todo-{}", todo.id)
    aria_label=tr("todos.toggle_done")
/>
```
(Adapt `toggle_done`/the toggle closure to the row's existing logic; keep the surrounding row markup. Add the `todos.toggle_done` key to `en.json` if absent.)

- [ ] **Step 3: Apply across the file list.** Build after each file.

- [ ] **Step 4: i18n** — add any missing English keys for new labels/aria-labels to `crates/i18n/locales/en.json`. No hardcoded strings.

- [ ] **Step 5: Build + fmt + release build** (gates). Expected PASS.

- [ ] **Step 6: Visual smoke** — modals' fields look identical; the todo checkbox toggles via click AND keyboard (Space); labels read correctly; `ar`/`tr` OK.

- [ ] **Step 7: Commit**

```bash
SKIP=commitizen git commit --no-verify -m "♻️ Wrap form inputs in Field; replace faked todo checkbox with Checkbox"
```

---

## Task A.6: Build the `Select` primitive (styled native)

**Files:**
- Create: `crates/spa/src/components/ui/select.rs`
- Modify: `crates/spa/src/components/ui/mod.rs`

- [ ] **Step 1: Write `crates/spa/src/components/ui/select.rs`**

Wraps a native `<select>` (keeps native accessibility + OS keyboard) with the brutalism styling and a custom chevron via a positioned container. Options are passed as children (`<option>`s), so callers keep full control.

```rust
use leptos::prelude::*;
use tw_merge::tw_merge;

/// Styled wrapper around a native `<select>` (keeps native a11y + OS
/// keyboard). The caller provides `<option>` children and an `on_change`.
#[component]
pub fn Select(
    /// Current value, bound to the native select via `prop:value`.
    #[prop(into)] value: Signal<String>,
    /// Fired with the new value on change.
    on_change: impl Fn(String) + 'static,
    #[prop(into, optional)] aria_label: MaybeProp<String>,
    #[prop(into, optional, default = String::new())] class: String,
    children: Children,
) -> impl IntoView {
    let merged = tw_merge!(
        "appearance-none border-2 border-ink rounded-xs pl-3 pr-8 py-1.5 text-sm \
         bg-transparent cursor-pointer outline-hidden \
         focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-teal",
        class
    );
    view! {
        <div class="relative inline-flex items-center">
            <select
                class=merged
                aria-label=move || aria_label.get()
                prop:value=value
                on:change=move |ev| on_change(event_target_value(&ev))
            >
                {children()}
            </select>
            // Custom chevron; pointer-events-none so clicks hit the select.
            <span aria-hidden="true"
                class="pointer-events-none absolute right-2 text-ink select-none">
                "▾"
            </span>
        </div>
    }
}
```

- [ ] **Step 2: Export — add `pub mod select;` to `ui/mod.rs`.**

- [ ] **Step 3: Build + fmt** (gates). Expected PASS.

- [ ] **Step 4: Commit**

```bash
SKIP=commitizen git commit --no-verify -m "✨ Add Select primitive (styled native select)"
```

---

## Task A.7: Convert the 5 native selects to `Select`

**Files (modify; add `use crate::components::ui::select::Select;`):**
- `crates/spa/src/pages/settings.rs` (`96-117` workspace language; `135-143` persona)
- `crates/spa/src/pages/memory.rs` (`242-252` kind filter)
- `crates/spa/src/pages/scheduled.rs` (`254-264` action-type filter)
- `crates/spa/src/pages/global_settings.rs` (`66-72` default locale)

- [ ] **Step 1: Worked example — persona select (`settings.rs:135-143`)**

After:
```rust
<Select
    value=persona
    on_change=move |v| set_persona.set(v)
    aria_label=tr("settings.row.persona")
>
    <option value="grumps">{move || tr("settings.persona.grumps")}</option>
    <option value="assistant">{move || tr("settings.persona.assistant")}</option>
    <option value="coach">{move || tr("settings.persona.coach")}</option>
</Select>
```
For the workspace-language select, keep the existing async `on:change` body inside `on_change=move |new_locale| { … spawn_local … }`. Keep the `Locale::ALL` option-rendering children verbatim.

- [ ] **Step 2: Apply to all 5 sites.** Build after each.

- [ ] **Step 3: Build + fmt + release build** (gates). Expected PASS. Confirm the generated CSS includes `.appearance-none` and the select renders with the custom chevron.

- [ ] **Step 4: Visual smoke** — each select looks consistent, opens natively, chevron positioned correctly in `ar` (RTL — the chevron should sit on the inline-end; if `right-2` looks wrong in RTL, switch to a logical `end-2` utility).

- [ ] **Step 5: Commit**

```bash
SKIP=commitizen git commit --no-verify -m "♻️ Convert native selects to the Select primitive"
```

---

## Task A.8: Phase A green gate + PR

- [ ] **Step 1: Full gates**

Run: format gate (`cargo fmt --all --check`), then `cd crates/spa && PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe build --target wasm32-unknown-unknown`, then `MSYS_NO_PATHCONV=1 trunk build --release`. Also run the workspace tests from root: `PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe test --target x86_64-pc-windows-msvc`. All PASS.

- [ ] **Step 2: i18n parity** — if new keys were added to `en.json`, run the repo's i18n validation (the CI `i18n locale parity` job; locally whatever `scripts/` provides) or at minimum confirm the new keys exist in `en.json`. New non-English locales can be backfilled later but English must be present.

- [ ] **Step 3: Push + PR**

```bash
git push -u origin feat/spa-ui-primitives
gh pr create --title "feat(spa): UI primitives foundation — Button, Field, Checkbox, Select" --body-file <a file you write to .git/PR_BODY_tmp.md, then delete it>
```
PR title is conventional-commits (squash-commit subject on `main`). Base the PR on `main` only after PR #5 has merged; otherwise target `feat/spa-leptos08-rust-ui` or note the dependency in the PR body.

---

## Self-review notes (author)

- **Spec coverage (Phase A scope):** `ui/` submodule + Switch move (A.1) ✓; Button (A.2–A.3) ✓; Field + Checkbox (A.4–A.5) ✓; Select styled-native (A.6–A.7) ✓; i18n discipline for new strings (A.3/A.5 steps) ✓; fmt gate included everywhere (lesson from the prior rustfmt CI failure) ✓. Dialog/DropdownMenu/Tabs/Toast are explicitly out of Phase A (Phases B–E, separate plans).
- **Type consistency:** `Button.on_click: impl Fn()`, `Checkbox`/`Select` use `Signal<bool>`/`Signal<String>` via `#[prop(into)]` (so `ReadSignal`/`RwSignal`/`Signal::derive` all coerce); `MaybeProp<String>` for optional reactive labels mirrors the shipped Switch fix. Module path `crate::components::ui::<name>::<Component>` is consistent across tasks.
- **No placeholders:** primitives are given in full; the conversions are a mechanical sweep with a worked example per control type + the exact site inventory — the correct shape for a 40+-site refactor (the implementer reads each call site and applies the pattern, building after each file).
- **Visual-fidelity guard:** every conversion task ends with a visual smoke step (default + `ar` + `tr`) to catch drift, since "no redesign" is a hard constraint.
</content>
