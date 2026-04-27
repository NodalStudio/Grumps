# API trait refactor — SPA demo/live split

**Status:** design ratified, ready for implementation plan
**Audit reference:** [`docs/superpowers/2026-04-26-audit-followups.md`](../2026-04-26-audit-followups.md) item #11
**Date:** 2026-04-27

## Why

`crates/spa/src/api.rs` holds 30+ public async methods on `ApiClient`. 29 of them carry an inline `if crate::demo::is_demo() { return ...; }` short-circuit so the demo iframe (`?demo=1`) gets seed data instead of HTTP. Every new API method is a fresh chance to forget the guard — the demo silently falls through to a real network call that won't work, the iframe shows a broken page, and there's no compile-time check that the demo path was covered.

The fix is to make the demo coverage type-checked: a `trait Api` with a `LiveApi` and `DemoApi` impl. Forgetting a demo branch becomes a missing trait method, which is a compile error.

## Non-goals

- **No error-type upgrade.** Methods keep `Result<T, String>`. Typed errors are worth their own audit pass; bundling them here doubles the diff for orthogonal value.
- **No demo-mode toggle change.** The `?demo=1` URL flag stays the runtime switch; demo stays runtime-toggleable, not compile-time.
- **No new tests.** The SPA has no unit tests today (Leptos + WASM is hard to unit-test). `DemoApi` becomes the de facto integration fixture, exercised by the demo iframe.
- **No request/response signature changes.** Trait methods keep the current `ApiClient` signatures verbatim.

## Architecture

### File layout

```
crates/spa/src/api/
  mod.rs       Trait Api (~30 async methods), Rc<dyn Api> typedef,
               provide_api() / use_api() context helpers, api_base().
  types.rs     Request/response structs moved verbatim from current api.rs
               (TodoItem, NoteItem, WorkspaceInfo, StatusCounts, MemberItem,
               ActivityItem, MemoryItem, EventItem, ScheduledActionItem,
               CalendarItem, WorkspaceSettings, ICalTokenResponse,
               ObservabilityData, GlobalObservabilityData,
               GlobalWorkspaceStats, GlobalModelCostAgg,
               QualitySignalCount, GlobalError, ObservabilityData fields,
               OtpResponse, VerifyResponse, AdminMe).
  live.rs      LiveApi unit struct + the trait impl (HTTP via gloo_net,
               cookies + CSRF). Private helpers build_get / build_with_csrf
               / get / post / put / patch / delete move here.
  demo.rs      DemoApi unit struct + the trait impl. Each method either
               delegates to a crate::demo::* function, returns Ok(())
               for unmodelled mutations, or returns Err for auth methods.

crates/spa/src/api.rs     DELETED — replaced by the api/ directory.

crates/spa/src/app.rs     Calls api::provide_api() once on startup,
                          before AuthGate runs.

crates/spa/src/{components,pages}/*.rs    Replace
                          let api = ApiClient::new();
                          with
                          let api = use_api();
                          (13 files, ~2 lines each).
```

### Trait technique

`#[async_trait::async_trait(?Send)] pub trait Api`. Consistent with the `messaging` and `agent` crates which already use `async_trait` with `?Send` bounds. The dependency `async-trait = "0.1"` is added to `crates/spa/Cargo.toml` (currently absent there; the workspace already has it in `Cargo.lock` via other crates).

### Pointer type

`pub type ApiHandle = Rc<dyn Api>;`

`Rc` not `Arc` — Leptos on WASM is single-threaded. `Rc<dyn Api>` is `Clone`, so the existing `let api2 = api.clone();` pattern in callsites keeps working without ceremony.

### Demo-mode toggle

Unchanged. `crate::demo::is_demo()` still reads the runtime URL/state. `provide_api()` checks it once at startup and constructs the matching impl:

```rust
pub fn provide_api() {
    let api: ApiHandle = if crate::demo::is_demo() {
        Rc::new(DemoApi)
    } else {
        Rc::new(LiveApi)
    };
    leptos::prelude::provide_context(api);
}

pub fn use_api() -> ApiHandle {
    leptos::prelude::expect_context::<ApiHandle>()
}
```

## Components

### `trait Api`

One `async fn` per current public method on `ApiClient`. Same signature, same return type. About 30 methods grouped by domain — listing categories, not every method:

- **Auth:** `send_otp`, `verify_otp`
- **Workspaces:** `get_workspaces`, `get_workspace_info`
- **Todos:** `get_todos`, `create_todo`, `update_todo`, `delete_todo`
- **Notes:** `get_notes`, `get_note`, `create_note`, `update_note`, `delete_note`
- **History / Members:** `get_history`, `get_members`
- **Memory:** `list_memory`, `create_memory`, `update_memory`, `delete_memory`
- **Events:** `list_events`, `create_event`, `update_event`, `delete_event`
- **Scheduled:** `list_scheduled_actions`, `create_scheduled_action`, `update_scheduled_action`, `delete_scheduled_action`
- **Calendar / Settings:** `list_calendar`, `get_settings`, `update_settings`, `update_workspace_locale`, `regenerate_ical_token`
- **Observability:** `get_observability`, `get_admin_me`, `get_global_observability`

Method visibility on the trait is `async fn ...(&self, ...) -> Result<T, String>` — `&self` because impls are unit structs but the trait shape allows future state.

### `LiveApi`

Unit struct (`pub struct LiveApi;`). Each method is the current `ApiClient::method` body minus the `if is_demo() { ... }` guard. The five private dispatch helpers (`build_get`, `build_with_csrf`, `get`, `post`, `put`, `patch`, `delete`) move from `ApiClient` to `LiveApi` as private associated functions. Cookies + CSRF semantics unchanged: every mutation reads `crate::auth::read_csrf_cookie()` and sets the `X-CSRF-Token` header.

### `DemoApi`

Unit struct (`pub struct DemoApi;`). Each trait method takes one of three shapes:

1. **Delegate to seed function** (the majority — ~22 methods). `get_todos` calls `crate::demo::todos()` and applies the filter; `get_workspaces` calls `crate::demo::workspaces()`; etc. Body is one or two lines.
2. **No-op success for unmodelled mutations.** Methods whose return type is `Result<(), String>` simply return `Ok(())` — the demo iframe doesn't persist state across reloads anyway. That covers `update_todo`, `delete_todo`, `update_note`, `delete_note`, `update_memory`, `delete_memory`, `update_event`, `delete_event`, `update_scheduled_action`, `delete_scheduled_action`, `update_settings`, `update_workspace_locale`. The one exception with a non-unit success type is `regenerate_ical_token`, which returns `Result<ICalTokenResponse, String>`; in demo mode it returns a synthesized `ICalTokenResponse { token: "demo-ical-token".into() }` so the callsite gets a renderable value.
3. **`Err` for auth.** `send_otp`, `verify_otp`, `get_admin_me` (and any future auth methods) return `Err("auth not available in demo mode".into())`. The demo gate skips these flows; an `Err` here surfaces a callsite bug rather than masking it.

### `crate::demo` module

Unchanged. Still owns `is_demo()`, `router_base()`, `install_postmessage_nav()`, `DEMO_TOKEN`, `DEMO_MEMBER_ID`, and the seed functions (`workspaces()`, `todos()`, `notes()`, `members()`, `events()`, `status_counts()`, `new_todo()`, `new_note()`, `new_event()`, …). `DemoApi` calls into them; nothing moves out of `crate::demo`.

## Data flow

### Startup (`app.rs`)

```rust
#[component]
pub fn App() -> impl IntoView {
    provide_locale();
    crate::api::provide_api();
    if crate::demo::is_demo() {
        crate::demo::install_postmessage_nav();
    }
    let base: String = crate::demo::router_base();
    view! { … }
}
```

### Callsite shape

Before (current after the AuthState removal in commit `1689912`):

```rust
use crate::api::ApiClient;
…
let api = ApiClient::new();
let res = LocalResource::new(move || {
    let api = api.clone();
    let s = slug();
    async move { api.get_todos(&s, "open").await.ok() }
});
```

After:

```rust
use crate::api::use_api;
…
let api = use_api();
let res = LocalResource::new(move || {
    let api = api.clone();
    let s = slug();
    async move { api.get_todos(&s, "open").await.ok() }
});
```

Diff per file: change one import, change one constructor call. The `.clone()`-into-closures pattern keeps working because `Rc<dyn Api>` is `Clone`. Affects 12 page files plus `components/sidebar.rs`.

## Migration mechanics

1. `git mv crates/spa/src/api.rs crates/spa/src/api/types.rs` (preserves blame on the request/response structs).
2. Strip the `ApiClient` impl out of `types.rs`, leaving only the `#[derive(Serialize, Deserialize)]` structs and the `api_base()` helper.
3. Add `crates/spa/src/api/mod.rs` with `pub mod types; pub mod live; pub mod demo;`, the `Api` trait, the `ApiHandle` typedef, `provide_api()` / `use_api()`, and `pub use types::*;` so external imports keep working.
4. Add `crates/spa/src/api/live.rs` with `LiveApi` + the trait impl moved from the old `ApiClient`.
5. Add `crates/spa/src/api/demo.rs` with `DemoApi` + the trait impl.
6. Add `async-trait = "0.1"` to `crates/spa/Cargo.toml` `[dependencies]`.
7. Update `crates/spa/src/app.rs` to call `crate::api::provide_api()`.
8. Sweep the 13 callsites: `let api = ApiClient::new();` → `let api = use_api();` and import `use_api` instead of `ApiClient`.
9. Compile-check: `cargo check --target wasm32-unknown-unknown -p grumps-spa`. Any missing trait method on `DemoApi` surfaces as a compile error — that's the audit's win.
10. Smoke-test: load the demo iframe locally, click through a few pages, confirm no broken renders.
11. Update `docs/superpowers/2026-04-26-audit-followups.md` to flip #11 from DEFERRED → ✅ with the date and commit reference.

## Error handling

Trait methods keep `Result<T, String>`. The single new `Err` site is `DemoApi`'s three auth methods (literal string `"auth not available in demo mode"`).

Failure modes:

- **`use_api()` called before `provide_api()` runs** → `expect_context` panics with a clear message. Same failure mode as today's `expect_context::<AuthState>()` was. Acceptable: a startup-order bug never reaches prod.
- **New `Api` method added to the trait, only `LiveApi` implements it** → compile error. This is the point of the refactor.
- **New method on `LiveApi` but not on the trait** → compile error at any callsite that tries `api.method(...)` on `Rc<dyn Api>`. Discoverable.

## Testing

No new tests. The SPA has no unit tests today (Leptos + WASM is hard to unit-test). Verification stays at:

- `cargo check --target wasm32-unknown-unknown -p grumps-spa` — must pass.
- `cargo clippy --target wasm32-unknown-unknown -p grumps-spa` — must pass.
- Manual demo iframe smoke test — load `?demo=1`, navigate Overview / Todos / Notes / Calendar / Memory / Scheduled / Settings / global observability, confirm no broken renders or console errors.

`DemoApi` itself becomes the integration fixture: the demo iframe loading in CI/local dev exercises every demo path.

## Risk register

| Risk | Mitigation |
|---|---|
| `cargo check` passes but demo iframe breaks at runtime due to a `DemoApi` method returning shape that doesn't match a callsite expectation | Manual smoke test of the demo iframe before merge. |
| `Rc<dyn Api>` lifetime issues in Leptos closures (move semantics) | `Rc::clone` (or `.clone()`) into each closure — same pattern callsites already use. |
| Trait method signature drift if migration is interrupted mid-PR | Big-bang strategy: single PR. If something goes wrong the revert is one `git reset`. |
| `async-trait`'s `?Send` mode produces awkward error messages on lifetime issues | Stick to `&self` borrows in trait methods; no method needs `&mut self`. |
| New SPA contributor misses the `provide_api()` call when adding a new top-level component | Documented in the file header of `api/mod.rs` and the `app.rs` startup code. |

## Out of scope

The following items are deliberately *not* part of this refactor; they're tracked separately or are YAGNI:

- Typed error type replacing `Result<T, String>` — separate audit follow-up.
- Splitting the `Api` trait by domain (`AuthApi`, `TodosApi`, …) — would need an aggregate trait or extension methods; complexity not justified by the audit's goal.
- Compile-time demo gating (`#[cfg(feature = "demo")]`) — would break the runtime `?demo=1` toggle.
- Unit tests for `LiveApi` / `DemoApi` — Leptos+WASM testing infra not in place; infra work is its own project.
- Refactoring `crate::demo` seed functions — they already have a coherent shape; moving them would be churn for no gain.
