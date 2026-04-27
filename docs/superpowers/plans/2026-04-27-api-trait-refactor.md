# API Trait Refactor — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the 35-method `ApiClient` (with its 29 inline `if is_demo()` short-circuits) by a `trait Api` and two impls (`LiveApi`, `DemoApi`), provided once at startup as `Rc<dyn Api>` via Leptos context. Forgetting a demo branch becomes a compile error.

**Architecture:** Single PR. `crates/spa/src/api.rs` → `crates/spa/src/api/{mod,types,live,demo}.rs`. The trait lives in `mod.rs` along with the `provide_api()` / `use_api()` helpers. `LiveApi` is the HTTP impl (current `ApiClient` body minus `is_demo` guards). `DemoApi` delegates to existing `crate::demo::*` seed functions; auth-only methods return `Err`. 13 callsites swap `ApiClient::new()` for `use_api()`.

**Tech Stack:** Rust 2021, Leptos 0.7 (CSR), `async-trait = "0.1"` (newly added to spa), gloo-net for HTTP, `Rc<dyn Api>`.

**Spec reference:** [`docs/superpowers/specs/2026-04-27-api-trait-refactor-design.md`](../specs/2026-04-27-api-trait-refactor-design.md)

---

## File Structure

**Created:**
- `crates/spa/src/api/mod.rs` — `trait Api` (35 methods), `ApiHandle`, `provide_api`, `use_api`. Re-exports types and impl modules.
- `crates/spa/src/api/types.rs` — All `#[derive(Serialize, Deserialize)]` request/response structs (moved verbatim).
- `crates/spa/src/api/live.rs` — `pub struct LiveApi;` + `#[async_trait(?Send)] impl Api for LiveApi`. HTTP via gloo-net.
- `crates/spa/src/api/demo.rs` — `pub struct DemoApi;` + `#[async_trait(?Send)] impl Api for DemoApi`. Delegates to `crate::demo::*`.

**Modified:**
- `crates/spa/Cargo.toml` — add `async-trait = "0.1"`.
- `crates/spa/src/app.rs` — call `crate::api::provide_api()` once on startup.
- 13 callsites — change `let api = ApiClient::new();` to `let api = use_api();` and update import.

**Deleted:**
- `crates/spa/src/api.rs` — replaced by the `api/` directory.

**Unchanged:**
- `crates/spa/src/demo.rs` — keeps all seed functions and runtime helpers.

---

## Task 1: Add `async-trait` dependency

**Files:**
- Modify: `crates/spa/Cargo.toml`

- [ ] **Step 1: Edit `crates/spa/Cargo.toml`, add `async-trait` after `gloo-net`**

```toml
[dependencies]
grumps-i18n = { path = "../i18n" }
leptos = { version = "0.7", features = ["csr"] }
leptos_router = "0.7"
gloo-net = { version = "0.6", features = ["http"] }
async-trait = "0.1"
gloo-storage = "0.3"
gloo-timers = { version = "0.3", features = ["futures"] }
```

- [ ] **Step 2: Verify build still passes**

Run: `CARGO_TARGET_DIR=C:/Users/mayer/Documents/Grumps/target PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe check --target wasm32-unknown-unknown -p grumps-spa --message-format=short 2>&1 | tail -3`

Expected: `Finished \`dev\` profile [unoptimized + debuginfo] target(s)` with no new errors. Pre-existing warnings (24, mostly `field never read`) remain unchanged.

- [ ] **Step 3: Commit**

```bash
git add crates/spa/Cargo.toml Cargo.lock
git commit -m "build(spa): add async-trait dependency for Api trait refactor"
```

---

## Task 2: Move `api.rs` into `api/mod.rs`

The directory move preserves git blame on the response types and method bodies. Pure file relocation, no content change.

**Files:**
- Move: `crates/spa/src/api.rs` → `crates/spa/src/api/mod.rs`

- [ ] **Step 1: Create the directory and move the file**

```bash
mkdir -p crates/spa/src/api
git mv crates/spa/src/api.rs crates/spa/src/api/mod.rs
```

- [ ] **Step 2: Verify build still passes**

Run: `CARGO_TARGET_DIR=C:/Users/mayer/Documents/Grumps/target PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe check --target wasm32-unknown-unknown -p grumps-spa --message-format=short 2>&1 | tail -3`

Expected: `Finished` — Rust resolves `mod api;` to either `api.rs` or `api/mod.rs`, so all 13 callsites that say `use crate::api::ApiClient;` keep working without change.

- [ ] **Step 3: Commit**

```bash
git add crates/spa/src/api/
git commit -m "refactor(spa): move api.rs into api/ module directory

Pure file relocation, no content change. Sets up the directory for
the upcoming Api trait + LiveApi/DemoApi split."
```

---

## Task 3: Extract response types into `api/types.rs`

Move every `#[derive(Serialize, Deserialize)]` struct out of `api/mod.rs` into a dedicated `types.rs`. The structs stay public via re-export.

**Files:**
- Create: `crates/spa/src/api/types.rs`
- Modify: `crates/spa/src/api/mod.rs`

- [ ] **Step 1: Create `crates/spa/src/api/types.rs`**

Copy the contents of these regions from `api/mod.rs` into a new `types.rs`:
- The `api_base()` function (currently lines 5-20 of `api/mod.rs`).
- All `#[derive(Serialize, Deserialize)]` struct definitions: `TodoItem`, `NoteItem`, `MemberItem`, `ActivityItem`, `WorkspaceInfo`, `StatusCounts`, `WorkspaceOverview`, `MemoryItem`, `EventItem`, `ScheduledActionItem`, `CalendarItem`, `WorkspaceSettings`, `ICalTokenResponse`, `OtpResponse`, `VerifyResponse`, `LlmCostByModel`, `LlmLatencyByModel`, `LlmInvocationCount`, `LlmErrorEntry`, `QualitySignalCount`, `CascadeEfficiency`, `ObservabilityData`, `AdminMe`, `GlobalWorkspaceStats`, `GlobalModelCostAgg`, `GlobalError`, `GlobalObservabilityData`.

Top of `types.rs`:

```rust
//! Request/response DTOs shared between the live HTTP impl and the demo
//! seed impl. Pure data — no client logic.

use serde::{Deserialize, Serialize};

pub fn api_base() -> String {
    web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .map(|origin| {
            if origin.contains("localhost") || origin.contains("127.0.0.1") {
                String::new()
            } else {
                origin.replace("grumps.app", "api.grumps.app")
            }
        })
        .unwrap_or_default()
}

// (… all the struct definitions follow, copied verbatim …)
```

- [ ] **Step 2: Strip the same content out of `api/mod.rs`**

Remove `api_base()` and every struct from `api/mod.rs`. Add at the top of `api/mod.rs`:

```rust
pub mod types;
pub use types::*;
```

The `ApiClient` struct + impl block stays in `mod.rs` for now (it's removed in Task 9).

- [ ] **Step 3: Verify build still passes**

Run: `CARGO_TARGET_DIR=C:/Users/mayer/Documents/Grumps/target PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe check --target wasm32-unknown-unknown -p grumps-spa --message-format=short 2>&1 | tail -3`

Expected: `Finished`. The `pub use types::*;` re-export means callsites that say `use crate::api::TodoItem;` still resolve.

- [ ] **Step 4: Commit**

```bash
git add crates/spa/src/api/types.rs crates/spa/src/api/mod.rs
git commit -m "refactor(spa): extract api response types into types.rs

Move every #[derive(Serialize, Deserialize)] struct + api_base()
out of api/mod.rs into api/types.rs. Re-exported via 'pub use
types::*;' so existing 'use crate::api::TodoItem;' imports keep
working."
```

---

## Task 4: Define `Api` trait + `ApiHandle` + context helpers

Add the trait, the `Rc<dyn Api>` typedef, and `provide_api` / `use_api` to `api/mod.rs`. The trait has zero impls at this point — just defined and unused. Compile passes.

**Files:**
- Modify: `crates/spa/src/api/mod.rs`

- [ ] **Step 1: Add the trait, typedef, and helpers at the top of `api/mod.rs`**

Insert this block immediately after `pub use types::*;` and before the existing `ApiClient` struct:

```rust
use std::rc::Rc;

/// Shared SPA API client interface. One impl for live HTTP (`LiveApi`),
/// one for demo seed data (`DemoApi`). The choice between them happens
/// once at startup based on `crate::demo::is_demo()`.
///
/// The trait makes demo coverage type-checked: every method must be
/// implemented by both impls, so a forgotten demo branch becomes a
/// compile error rather than a silent fall-through to the network.
#[async_trait::async_trait(?Send)]
pub trait Api {
    // Auth
    async fn send_otp(&self, phone: &str) -> Result<OtpResponse, String>;
    async fn verify_otp(&self, phone: &str, code: &str) -> Result<VerifyResponse, String>;

    // Workspaces
    async fn get_workspaces(&self) -> Result<Vec<WorkspaceInfo>, String>;
    async fn get_workspace_info(&self, slug: &str) -> Result<WorkspaceOverview, String>;

    // Todos
    async fn get_todos(&self, slug: &str, filter: &str) -> Result<Vec<TodoItem>, String>;
    async fn create_todo(&self, slug: &str, title: &str, priority: i32) -> Result<TodoItem, String>;
    async fn update_todo(&self, slug: &str, id: &str, updates: &serde_json::Value) -> Result<(), String>;
    async fn delete_todo(&self, slug: &str, id: &str) -> Result<(), String>;

    // Notes
    async fn get_notes(&self, slug: &str) -> Result<Vec<NoteItem>, String>;
    async fn get_note(&self, slug: &str, id: &str) -> Result<NoteItem, String>;
    async fn create_note(&self, slug: &str, title: &str, content: &str) -> Result<NoteItem, String>;
    async fn update_note(&self, slug: &str, id: &str, title: &str, content: &str) -> Result<(), String>;
    async fn delete_note(&self, slug: &str, id: &str) -> Result<(), String>;

    // History + Members
    async fn get_history(&self, slug: &str) -> Result<Vec<ActivityItem>, String>;
    async fn get_members(&self, slug: &str) -> Result<Vec<MemberItem>, String>;

    // Memory
    async fn list_memory(&self, slug: &str) -> Result<Vec<MemoryItem>, String>;
    async fn create_memory(&self, slug: &str, body: &serde_json::Value) -> Result<MemoryItem, String>;
    async fn update_memory(&self, slug: &str, id: &str, body: &serde_json::Value) -> Result<(), String>;
    async fn delete_memory(&self, slug: &str, id: &str) -> Result<(), String>;

    // Events
    async fn list_events(&self, slug: &str) -> Result<Vec<EventItem>, String>;
    async fn create_event(&self, slug: &str, body: &serde_json::Value) -> Result<EventItem, String>;
    async fn update_event(&self, slug: &str, id: &str, body: &serde_json::Value) -> Result<(), String>;
    async fn delete_event(&self, slug: &str, id: &str) -> Result<(), String>;

    // Scheduled actions
    async fn list_scheduled_actions(&self, slug: &str) -> Result<Vec<ScheduledActionItem>, String>;
    async fn create_scheduled_action(&self, slug: &str, body: &serde_json::Value) -> Result<ScheduledActionItem, String>;
    async fn update_scheduled_action(&self, slug: &str, id: &str, body: &serde_json::Value) -> Result<(), String>;
    async fn delete_scheduled_action(&self, slug: &str, id: &str) -> Result<(), String>;

    // Calendar + Settings
    async fn list_calendar(&self, slug: &str, from: &str, to: &str) -> Result<Vec<CalendarItem>, String>;
    async fn get_settings(&self, slug: &str) -> Result<WorkspaceSettings, String>;
    async fn update_settings(&self, slug: &str, body: &serde_json::Value) -> Result<(), String>;
    async fn update_workspace_locale(&self, slug: &str, locale: &str) -> Result<(), String>;
    async fn regenerate_ical_token(&self, slug: &str) -> Result<ICalTokenResponse, String>;

    // Observability + Admin
    async fn get_observability(&self, slug: &str) -> Result<ObservabilityData, String>;
    async fn get_admin_me(&self) -> Result<AdminMe, String>;
    async fn get_global_observability(&self) -> Result<GlobalObservabilityData, String>;
}

/// Shared handle to whichever Api impl is in use for this session.
pub type ApiHandle = Rc<dyn Api>;

/// Construct the right Api impl for the current mode and stash it in
/// Leptos context. Call once from `App` at startup, before any
/// component reads it via `use_api()`.
pub fn provide_api() {
    let api: ApiHandle = if crate::demo::is_demo() {
        Rc::new(demo::DemoApi)
    } else {
        Rc::new(live::LiveApi)
    };
    leptos::prelude::provide_context(api);
}

/// Fetch the Api handle from Leptos context. Panics if `provide_api`
/// was never called — same failure mode as any unprovided context.
pub fn use_api() -> ApiHandle {
    leptos::prelude::expect_context::<ApiHandle>()
}

pub mod live;
pub mod demo;
```

The `pub mod live; pub mod demo;` declarations point to files that don't exist yet — Step 2 stubs them.

- [ ] **Step 2: Create empty `api/live.rs` and `api/demo.rs` stubs so the module declarations resolve**

`crates/spa/src/api/live.rs`:

```rust
//! Live HTTP impl of the Api trait. Bodies move here from the legacy
//! ApiClient struct in Task 5.

use crate::api::Api;

pub struct LiveApi;

#[async_trait::async_trait(?Send)]
impl Api for LiveApi {
    // Method bodies added in Task 5.
    async fn send_otp(&self, _phone: &str) -> Result<crate::api::OtpResponse, String> {
        unimplemented!("filled in Task 5")
    }
    async fn verify_otp(&self, _phone: &str, _code: &str) -> Result<crate::api::VerifyResponse, String> {
        unimplemented!("filled in Task 5")
    }
    async fn get_workspaces(&self) -> Result<Vec<crate::api::WorkspaceInfo>, String> { unimplemented!() }
    async fn get_workspace_info(&self, _slug: &str) -> Result<crate::api::WorkspaceOverview, String> { unimplemented!() }
    async fn get_todos(&self, _slug: &str, _filter: &str) -> Result<Vec<crate::api::TodoItem>, String> { unimplemented!() }
    async fn create_todo(&self, _slug: &str, _title: &str, _priority: i32) -> Result<crate::api::TodoItem, String> { unimplemented!() }
    async fn update_todo(&self, _slug: &str, _id: &str, _updates: &serde_json::Value) -> Result<(), String> { unimplemented!() }
    async fn delete_todo(&self, _slug: &str, _id: &str) -> Result<(), String> { unimplemented!() }
    async fn get_notes(&self, _slug: &str) -> Result<Vec<crate::api::NoteItem>, String> { unimplemented!() }
    async fn get_note(&self, _slug: &str, _id: &str) -> Result<crate::api::NoteItem, String> { unimplemented!() }
    async fn create_note(&self, _slug: &str, _title: &str, _content: &str) -> Result<crate::api::NoteItem, String> { unimplemented!() }
    async fn update_note(&self, _slug: &str, _id: &str, _title: &str, _content: &str) -> Result<(), String> { unimplemented!() }
    async fn delete_note(&self, _slug: &str, _id: &str) -> Result<(), String> { unimplemented!() }
    async fn get_history(&self, _slug: &str) -> Result<Vec<crate::api::ActivityItem>, String> { unimplemented!() }
    async fn get_members(&self, _slug: &str) -> Result<Vec<crate::api::MemberItem>, String> { unimplemented!() }
    async fn list_memory(&self, _slug: &str) -> Result<Vec<crate::api::MemoryItem>, String> { unimplemented!() }
    async fn create_memory(&self, _slug: &str, _body: &serde_json::Value) -> Result<crate::api::MemoryItem, String> { unimplemented!() }
    async fn update_memory(&self, _slug: &str, _id: &str, _body: &serde_json::Value) -> Result<(), String> { unimplemented!() }
    async fn delete_memory(&self, _slug: &str, _id: &str) -> Result<(), String> { unimplemented!() }
    async fn list_events(&self, _slug: &str) -> Result<Vec<crate::api::EventItem>, String> { unimplemented!() }
    async fn create_event(&self, _slug: &str, _body: &serde_json::Value) -> Result<crate::api::EventItem, String> { unimplemented!() }
    async fn update_event(&self, _slug: &str, _id: &str, _body: &serde_json::Value) -> Result<(), String> { unimplemented!() }
    async fn delete_event(&self, _slug: &str, _id: &str) -> Result<(), String> { unimplemented!() }
    async fn list_scheduled_actions(&self, _slug: &str) -> Result<Vec<crate::api::ScheduledActionItem>, String> { unimplemented!() }
    async fn create_scheduled_action(&self, _slug: &str, _body: &serde_json::Value) -> Result<crate::api::ScheduledActionItem, String> { unimplemented!() }
    async fn update_scheduled_action(&self, _slug: &str, _id: &str, _body: &serde_json::Value) -> Result<(), String> { unimplemented!() }
    async fn delete_scheduled_action(&self, _slug: &str, _id: &str) -> Result<(), String> { unimplemented!() }
    async fn list_calendar(&self, _slug: &str, _from: &str, _to: &str) -> Result<Vec<crate::api::CalendarItem>, String> { unimplemented!() }
    async fn get_settings(&self, _slug: &str) -> Result<crate::api::WorkspaceSettings, String> { unimplemented!() }
    async fn update_settings(&self, _slug: &str, _body: &serde_json::Value) -> Result<(), String> { unimplemented!() }
    async fn update_workspace_locale(&self, _slug: &str, _locale: &str) -> Result<(), String> { unimplemented!() }
    async fn regenerate_ical_token(&self, _slug: &str) -> Result<crate::api::ICalTokenResponse, String> { unimplemented!() }
    async fn get_observability(&self, _slug: &str) -> Result<crate::api::ObservabilityData, String> { unimplemented!() }
    async fn get_admin_me(&self) -> Result<crate::api::AdminMe, String> { unimplemented!() }
    async fn get_global_observability(&self) -> Result<crate::api::GlobalObservabilityData, String> { unimplemented!() }
}
```

`crates/spa/src/api/demo.rs`:

```rust
//! Demo seed-data impl of the Api trait. Delegates to the existing
//! crate::demo::* helper functions. Bodies added in Task 6.

use crate::api::Api;

pub struct DemoApi;

#[async_trait::async_trait(?Send)]
impl Api for DemoApi {
    async fn send_otp(&self, _phone: &str) -> Result<crate::api::OtpResponse, String> { unimplemented!() }
    async fn verify_otp(&self, _phone: &str, _code: &str) -> Result<crate::api::VerifyResponse, String> { unimplemented!() }
    async fn get_workspaces(&self) -> Result<Vec<crate::api::WorkspaceInfo>, String> { unimplemented!() }
    async fn get_workspace_info(&self, _slug: &str) -> Result<crate::api::WorkspaceOverview, String> { unimplemented!() }
    async fn get_todos(&self, _slug: &str, _filter: &str) -> Result<Vec<crate::api::TodoItem>, String> { unimplemented!() }
    async fn create_todo(&self, _slug: &str, _title: &str, _priority: i32) -> Result<crate::api::TodoItem, String> { unimplemented!() }
    async fn update_todo(&self, _slug: &str, _id: &str, _updates: &serde_json::Value) -> Result<(), String> { unimplemented!() }
    async fn delete_todo(&self, _slug: &str, _id: &str) -> Result<(), String> { unimplemented!() }
    async fn get_notes(&self, _slug: &str) -> Result<Vec<crate::api::NoteItem>, String> { unimplemented!() }
    async fn get_note(&self, _slug: &str, _id: &str) -> Result<crate::api::NoteItem, String> { unimplemented!() }
    async fn create_note(&self, _slug: &str, _title: &str, _content: &str) -> Result<crate::api::NoteItem, String> { unimplemented!() }
    async fn update_note(&self, _slug: &str, _id: &str, _title: &str, _content: &str) -> Result<(), String> { unimplemented!() }
    async fn delete_note(&self, _slug: &str, _id: &str) -> Result<(), String> { unimplemented!() }
    async fn get_history(&self, _slug: &str) -> Result<Vec<crate::api::ActivityItem>, String> { unimplemented!() }
    async fn get_members(&self, _slug: &str) -> Result<Vec<crate::api::MemberItem>, String> { unimplemented!() }
    async fn list_memory(&self, _slug: &str) -> Result<Vec<crate::api::MemoryItem>, String> { unimplemented!() }
    async fn create_memory(&self, _slug: &str, _body: &serde_json::Value) -> Result<crate::api::MemoryItem, String> { unimplemented!() }
    async fn update_memory(&self, _slug: &str, _id: &str, _body: &serde_json::Value) -> Result<(), String> { unimplemented!() }
    async fn delete_memory(&self, _slug: &str, _id: &str) -> Result<(), String> { unimplemented!() }
    async fn list_events(&self, _slug: &str) -> Result<Vec<crate::api::EventItem>, String> { unimplemented!() }
    async fn create_event(&self, _slug: &str, _body: &serde_json::Value) -> Result<crate::api::EventItem, String> { unimplemented!() }
    async fn update_event(&self, _slug: &str, _id: &str, _body: &serde_json::Value) -> Result<(), String> { unimplemented!() }
    async fn delete_event(&self, _slug: &str, _id: &str) -> Result<(), String> { unimplemented!() }
    async fn list_scheduled_actions(&self, _slug: &str) -> Result<Vec<crate::api::ScheduledActionItem>, String> { unimplemented!() }
    async fn create_scheduled_action(&self, _slug: &str, _body: &serde_json::Value) -> Result<crate::api::ScheduledActionItem, String> { unimplemented!() }
    async fn update_scheduled_action(&self, _slug: &str, _id: &str, _body: &serde_json::Value) -> Result<(), String> { unimplemented!() }
    async fn delete_scheduled_action(&self, _slug: &str, _id: &str) -> Result<(), String> { unimplemented!() }
    async fn list_calendar(&self, _slug: &str, _from: &str, _to: &str) -> Result<Vec<crate::api::CalendarItem>, String> { unimplemented!() }
    async fn get_settings(&self, _slug: &str) -> Result<crate::api::WorkspaceSettings, String> { unimplemented!() }
    async fn update_settings(&self, _slug: &str, _body: &serde_json::Value) -> Result<(), String> { unimplemented!() }
    async fn update_workspace_locale(&self, _slug: &str, _locale: &str) -> Result<(), String> { unimplemented!() }
    async fn regenerate_ical_token(&self, _slug: &str) -> Result<crate::api::ICalTokenResponse, String> { unimplemented!() }
    async fn get_observability(&self, _slug: &str) -> Result<crate::api::ObservabilityData, String> { unimplemented!() }
    async fn get_admin_me(&self) -> Result<crate::api::AdminMe, String> { unimplemented!() }
    async fn get_global_observability(&self) -> Result<crate::api::GlobalObservabilityData, String> { unimplemented!() }
}
```

- [ ] **Step 3: Verify build still passes**

Run: `CARGO_TARGET_DIR=C:/Users/mayer/Documents/Grumps/target PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe check --target wasm32-unknown-unknown -p grumps-spa --message-format=short 2>&1 | tail -3`

Expected: `Finished`. Trait + stub impls compile (callsites still use `ApiClient::new()` which is unchanged).

- [ ] **Step 4: Commit**

```bash
git add crates/spa/src/api/mod.rs crates/spa/src/api/live.rs crates/spa/src/api/demo.rs
git commit -m "feat(spa): introduce Api trait, ApiHandle, provide_api/use_api

Define the Api trait covering all 35 client methods (auth,
workspaces, todos, notes, history, members, memory, events,
scheduled, calendar, settings, observability, admin). Stub
LiveApi and DemoApi with unimplemented!() bodies — filled in
the next two tasks. The legacy ApiClient struct is unchanged
and still serves all 13 callsites; it gets removed once both
impls are populated."
```

---

## Task 5: Populate `LiveApi` impl with HTTP bodies

Move the HTTP method bodies and the private `build_get` / `build_with_csrf` / `get` / `post` / `put` / `patch` / `delete` helpers from the legacy `ApiClient` (in `api/mod.rs`) into `LiveApi` (in `api/live.rs`). Drop the `if is_demo()` guards — `DemoApi` will handle those.

**Files:**
- Modify: `crates/spa/src/api/live.rs`

- [ ] **Step 1: Replace `crates/spa/src/api/live.rs` contents in full**

```rust
//! Live HTTP impl of the Api trait. Cookies + CSRF carry auth.

use crate::api::*;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};

pub struct LiveApi;

impl LiveApi {
    fn build_get(url: &str) -> gloo_net::http::RequestBuilder {
        Request::get(url).credentials(web_sys::RequestCredentials::Include)
    }

    fn build_with_csrf(rb: gloo_net::http::RequestBuilder) -> gloo_net::http::RequestBuilder {
        rb.credentials(web_sys::RequestCredentials::Include)
            .header("X-CSRF-Token", &crate::auth::read_csrf_cookie())
    }

    async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T, String> {
        let url = format!("{}{}", api_base(), path);
        let resp = Self::build_get(&url).send().await.map_err(|e| e.to_string())?;
        if !resp.ok() {
            return Err(format!("HTTP {}: {}", resp.status(), resp.status_text()));
        }
        resp.json().await.map_err(|e| e.to_string())
    }

    async fn post<B: Serialize, T: for<'de> Deserialize<'de>>(&self, path: &str, body: &B) -> Result<T, String> {
        let url = format!("{}{}", api_base(), path);
        let req = Self::build_with_csrf(Request::post(&url)).header("Content-Type", "application/json");
        let resp = req.json(body).map_err(|e| e.to_string())?.send().await.map_err(|e| e.to_string())?;
        if !resp.ok() { return Err(format!("HTTP {}", resp.status())); }
        resp.json().await.map_err(|e| e.to_string())
    }

    async fn put<B: Serialize>(&self, path: &str, body: &B) -> Result<(), String> {
        let url = format!("{}{}", api_base(), path);
        let req = Self::build_with_csrf(Request::put(&url)).header("Content-Type", "application/json");
        let resp = req.json(body).map_err(|e| e.to_string())?.send().await.map_err(|e| e.to_string())?;
        if !resp.ok() { return Err(format!("HTTP {}", resp.status())); }
        Ok(())
    }

    async fn patch<B: Serialize>(&self, path: &str, body: &B) -> Result<(), String> {
        let url = format!("{}{}", api_base(), path);
        let req = Self::build_with_csrf(Request::patch(&url)).header("Content-Type", "application/json");
        let resp = req.json(body).map_err(|e| e.to_string())?.send().await.map_err(|e| e.to_string())?;
        if !resp.ok() { return Err(format!("HTTP {}", resp.status())); }
        Ok(())
    }

    async fn delete(&self, path: &str) -> Result<(), String> {
        let url = format!("{}{}", api_base(), path);
        let req = Self::build_with_csrf(Request::delete(&url));
        let resp = req.send().await.map_err(|e| e.to_string())?;
        if !resp.ok() { return Err(format!("HTTP {}", resp.status())); }
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Api for LiveApi {
    async fn send_otp(&self, phone: &str) -> Result<OtpResponse, String> {
        self.post("/auth/otp", &serde_json::json!({"phone": phone})).await
    }
    async fn verify_otp(&self, phone: &str, code: &str) -> Result<VerifyResponse, String> {
        self.post("/auth/verify", &serde_json::json!({"phone": phone, "code": code})).await
    }

    async fn get_workspaces(&self) -> Result<Vec<WorkspaceInfo>, String> {
        self.get("/api/workspaces").await
    }
    async fn get_workspace_info(&self, slug: &str) -> Result<WorkspaceOverview, String> {
        self.get(&format!("/api/w/{}", slug)).await
    }

    async fn get_todos(&self, slug: &str, filter: &str) -> Result<Vec<TodoItem>, String> {
        self.get(&format!("/api/w/{}/todos?status={}", slug, filter)).await
    }
    async fn create_todo(&self, slug: &str, title: &str, priority: i32) -> Result<TodoItem, String> {
        self.post(&format!("/api/w/{}/todos", slug), &serde_json::json!({"title": title, "priority": priority})).await
    }
    async fn update_todo(&self, slug: &str, id: &str, updates: &serde_json::Value) -> Result<(), String> {
        self.patch(&format!("/api/w/{}/todos/{}", slug, id), updates).await
    }
    async fn delete_todo(&self, slug: &str, id: &str) -> Result<(), String> {
        self.delete(&format!("/api/w/{}/todos/{}", slug, id)).await
    }

    async fn get_notes(&self, slug: &str) -> Result<Vec<NoteItem>, String> {
        self.get(&format!("/api/w/{}/notes", slug)).await
    }
    async fn get_note(&self, slug: &str, id: &str) -> Result<NoteItem, String> {
        self.get(&format!("/api/w/{}/notes/{}", slug, id)).await
    }
    async fn create_note(&self, slug: &str, title: &str, content: &str) -> Result<NoteItem, String> {
        self.post(&format!("/api/w/{}/notes", slug), &serde_json::json!({"title": title, "content": content})).await
    }
    async fn update_note(&self, slug: &str, id: &str, title: &str, content: &str) -> Result<(), String> {
        self.put(&format!("/api/w/{}/notes/{}", slug, id), &serde_json::json!({"title": title, "content": content})).await
    }
    async fn delete_note(&self, slug: &str, id: &str) -> Result<(), String> {
        self.delete(&format!("/api/w/{}/notes/{}", slug, id)).await
    }

    async fn get_history(&self, slug: &str) -> Result<Vec<ActivityItem>, String> {
        self.get(&format!("/api/w/{}/history", slug)).await
    }
    async fn get_members(&self, slug: &str) -> Result<Vec<MemberItem>, String> {
        self.get(&format!("/api/w/{}/members", slug)).await
    }

    async fn list_memory(&self, slug: &str) -> Result<Vec<MemoryItem>, String> {
        self.get(&format!("/api/w/{}/memory", slug)).await
    }
    async fn create_memory(&self, slug: &str, body: &serde_json::Value) -> Result<MemoryItem, String> {
        self.post(&format!("/api/w/{}/memory", slug), body).await
    }
    async fn update_memory(&self, slug: &str, id: &str, body: &serde_json::Value) -> Result<(), String> {
        self.patch(&format!("/api/w/{}/memory/{}", slug, id), body).await
    }
    async fn delete_memory(&self, slug: &str, id: &str) -> Result<(), String> {
        self.delete(&format!("/api/w/{}/memory/{}", slug, id)).await
    }

    async fn list_events(&self, slug: &str) -> Result<Vec<EventItem>, String> {
        self.get(&format!("/api/w/{}/events", slug)).await
    }
    async fn create_event(&self, slug: &str, body: &serde_json::Value) -> Result<EventItem, String> {
        self.post(&format!("/api/w/{}/events", slug), body).await
    }
    async fn update_event(&self, slug: &str, id: &str, body: &serde_json::Value) -> Result<(), String> {
        self.patch(&format!("/api/w/{}/events/{}", slug, id), body).await
    }
    async fn delete_event(&self, slug: &str, id: &str) -> Result<(), String> {
        self.delete(&format!("/api/w/{}/events/{}", slug, id)).await
    }

    async fn list_scheduled_actions(&self, slug: &str) -> Result<Vec<ScheduledActionItem>, String> {
        self.get(&format!("/api/w/{}/scheduled-actions", slug)).await
    }
    async fn create_scheduled_action(&self, slug: &str, body: &serde_json::Value) -> Result<ScheduledActionItem, String> {
        self.post(&format!("/api/w/{}/scheduled-actions", slug), body).await
    }
    async fn update_scheduled_action(&self, slug: &str, id: &str, body: &serde_json::Value) -> Result<(), String> {
        self.patch(&format!("/api/w/{}/scheduled-actions/{}", slug, id), body).await
    }
    async fn delete_scheduled_action(&self, slug: &str, id: &str) -> Result<(), String> {
        self.delete(&format!("/api/w/{}/scheduled-actions/{}", slug, id)).await
    }

    async fn list_calendar(&self, slug: &str, from: &str, to: &str) -> Result<Vec<CalendarItem>, String> {
        self.get(&format!("/api/w/{}/calendar?from={}&to={}", slug, from, to)).await
    }
    async fn get_settings(&self, slug: &str) -> Result<WorkspaceSettings, String> {
        self.get(&format!("/api/w/{}/settings", slug)).await
    }
    async fn update_settings(&self, slug: &str, body: &serde_json::Value) -> Result<(), String> {
        self.put(&format!("/api/w/{}/settings", slug), body).await
    }
    async fn update_workspace_locale(&self, slug: &str, locale: &str) -> Result<(), String> {
        self.patch(&format!("/api/w/{}/settings/locale", slug), &serde_json::json!({"locale": locale})).await
    }
    async fn regenerate_ical_token(&self, slug: &str) -> Result<ICalTokenResponse, String> {
        self.post(&format!("/api/w/{}/calendar/ical-token", slug), &serde_json::json!({})).await
    }

    async fn get_observability(&self, slug: &str) -> Result<ObservabilityData, String> {
        self.get(&format!("/api/w/{}/admin/observability", slug)).await
    }
    async fn get_admin_me(&self) -> Result<AdminMe, String> {
        self.get("/api/admin/me").await
    }
    async fn get_global_observability(&self) -> Result<GlobalObservabilityData, String> {
        self.get("/api/admin/observability").await
    }
}
```

- [ ] **Step 2: Verify build still passes**

Run: `CARGO_TARGET_DIR=C:/Users/mayer/Documents/Grumps/target PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe check --target wasm32-unknown-unknown -p grumps-spa --message-format=short 2>&1 | tail -3`

Expected: `Finished`. The legacy `ApiClient` in `api/mod.rs` still works for the 13 callsites; `LiveApi` is a parallel impl that no callsite uses yet.

- [ ] **Step 3: Commit**

```bash
git add crates/spa/src/api/live.rs
git commit -m "feat(spa): populate LiveApi with HTTP bodies

Move the 35 HTTP method bodies plus the five private dispatch
helpers (build_get, build_with_csrf, get, post, put, patch,
delete) from the legacy ApiClient into LiveApi. Drop the inline
'if is_demo()' guards — DemoApi handles those in the next task.

LiveApi is unused at this point; ApiClient still serves all 13
callsites until Task 8."
```

---

## Task 6: Populate `DemoApi` impl with seed-data delegations

For each method, either delegate to a `crate::demo::*` function, return `Ok(())` for unmodelled mutations, or return `Err` for auth methods.

**Files:**
- Modify: `crates/spa/src/api/demo.rs`

- [ ] **Step 1: Replace `crates/spa/src/api/demo.rs` contents in full**

```rust
//! Demo seed-data impl of the Api trait. Used when the SPA loads
//! with `?demo=1`. Delegates to the existing crate::demo::* helpers;
//! mutations not modelled by the seed return Ok(()); auth methods
//! return Err since auth is bypassed entirely in demo mode.

use crate::api::*;

pub struct DemoApi;

#[async_trait::async_trait(?Send)]
impl Api for DemoApi {
    // Auth — never reachable in demo because the gate skips the login
    // flow. Returning Err here surfaces a callsite bug rather than
    // masking it.
    async fn send_otp(&self, _phone: &str) -> Result<OtpResponse, String> {
        Err("auth not available in demo mode".into())
    }
    async fn verify_otp(&self, _phone: &str, _code: &str) -> Result<VerifyResponse, String> {
        Err("auth not available in demo mode".into())
    }

    async fn get_workspaces(&self) -> Result<Vec<WorkspaceInfo>, String> {
        Ok(crate::demo::workspaces())
    }
    async fn get_workspace_info(&self, slug: &str) -> Result<WorkspaceOverview, String> {
        Ok(WorkspaceOverview {
            slug: slug.into(),
            name: Some("seed.workspace.name".into()),
            plan: "free".into(),
            stats: crate::demo::status_counts(),
        })
    }

    async fn get_todos(&self, _slug: &str, filter: &str) -> Result<Vec<TodoItem>, String> {
        let mut items = crate::demo::todos();
        if filter != "all" { items.retain(|t| t.status == filter); }
        Ok(items)
    }
    async fn create_todo(&self, _slug: &str, title: &str, priority: i32) -> Result<TodoItem, String> {
        Ok(crate::demo::new_todo(title, priority))
    }
    async fn update_todo(&self, _slug: &str, _id: &str, _updates: &serde_json::Value) -> Result<(), String> {
        Ok(())
    }
    async fn delete_todo(&self, _slug: &str, _id: &str) -> Result<(), String> {
        Ok(())
    }

    async fn get_notes(&self, _slug: &str) -> Result<Vec<NoteItem>, String> {
        Ok(crate::demo::notes())
    }
    async fn get_note(&self, _slug: &str, id: &str) -> Result<NoteItem, String> {
        crate::demo::notes().into_iter()
            .find(|n| n.id == id)
            .ok_or_else(|| "demo: note not found".into())
    }
    async fn create_note(&self, _slug: &str, title: &str, content: &str) -> Result<NoteItem, String> {
        Ok(crate::demo::new_note(title, content))
    }
    async fn update_note(&self, _slug: &str, _id: &str, _title: &str, _content: &str) -> Result<(), String> {
        Ok(())
    }
    async fn delete_note(&self, _slug: &str, _id: &str) -> Result<(), String> {
        Ok(())
    }

    async fn get_history(&self, _slug: &str) -> Result<Vec<ActivityItem>, String> {
        Ok(crate::demo::activity())
    }
    async fn get_members(&self, _slug: &str) -> Result<Vec<MemberItem>, String> {
        Ok(crate::demo::members())
    }

    async fn list_memory(&self, _slug: &str) -> Result<Vec<MemoryItem>, String> {
        Ok(crate::demo::memories())
    }
    async fn create_memory(&self, _slug: &str, body: &serde_json::Value) -> Result<MemoryItem, String> {
        Ok(crate::demo::new_memory(body))
    }
    async fn update_memory(&self, _slug: &str, _id: &str, _body: &serde_json::Value) -> Result<(), String> {
        Ok(())
    }
    async fn delete_memory(&self, _slug: &str, _id: &str) -> Result<(), String> {
        Ok(())
    }

    async fn list_events(&self, _slug: &str) -> Result<Vec<EventItem>, String> {
        Ok(crate::demo::events())
    }
    async fn create_event(&self, _slug: &str, body: &serde_json::Value) -> Result<EventItem, String> {
        Ok(crate::demo::new_event(body))
    }
    async fn update_event(&self, _slug: &str, _id: &str, _body: &serde_json::Value) -> Result<(), String> {
        Ok(())
    }
    async fn delete_event(&self, _slug: &str, _id: &str) -> Result<(), String> {
        Ok(())
    }

    async fn list_scheduled_actions(&self, _slug: &str) -> Result<Vec<ScheduledActionItem>, String> {
        Ok(crate::demo::scheduled_actions())
    }
    async fn create_scheduled_action(&self, _slug: &str, body: &serde_json::Value) -> Result<ScheduledActionItem, String> {
        Ok(crate::demo::new_scheduled(body))
    }
    async fn update_scheduled_action(&self, _slug: &str, _id: &str, _body: &serde_json::Value) -> Result<(), String> {
        Ok(())
    }
    async fn delete_scheduled_action(&self, _slug: &str, _id: &str) -> Result<(), String> {
        Ok(())
    }

    async fn list_calendar(&self, _slug: &str, _from: &str, _to: &str) -> Result<Vec<CalendarItem>, String> {
        Ok(crate::demo::calendar_items())
    }
    async fn get_settings(&self, _slug: &str) -> Result<WorkspaceSettings, String> {
        Ok(crate::demo::settings())
    }
    async fn update_settings(&self, _slug: &str, _body: &serde_json::Value) -> Result<(), String> {
        Ok(())
    }
    async fn update_workspace_locale(&self, _slug: &str, _locale: &str) -> Result<(), String> {
        Ok(())
    }
    async fn regenerate_ical_token(&self, _slug: &str) -> Result<ICalTokenResponse, String> {
        Ok(ICalTokenResponse {
            token: "demo-ical-token".into(),
            url: "/demo/calendar.ics".into(),
        })
    }

    // Observability returns a hand-rolled empty payload — the demo
    // dashboard renders empty sections without crashing.
    async fn get_observability(&self, _slug: &str) -> Result<ObservabilityData, String> {
        Ok(ObservabilityData::default())
    }
    async fn get_admin_me(&self) -> Result<AdminMe, String> {
        Err("auth not available in demo mode".into())
    }
    async fn get_global_observability(&self) -> Result<GlobalObservabilityData, String> {
        Err("auth not available in demo mode".into())
    }
}
```

- [ ] **Step 2: Verify build still passes**

Run: `CARGO_TARGET_DIR=C:/Users/mayer/Documents/Grumps/target PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe check --target wasm32-unknown-unknown -p grumps-spa --message-format=short 2>&1 | tail -3`

Expected: `Finished`.

- [ ] **Step 3: Commit**

```bash
git add crates/spa/src/api/demo.rs
git commit -m "feat(spa): populate DemoApi with seed-data delegations

Each method either delegates to crate::demo::* (the bulk),
returns Ok(()) for mutations the seed doesn't model, or returns
Err for auth methods (which the demo gate skips entirely).
Trait coverage is now complete for both impls — forgetting a
demo branch on a future method becomes a compile error."
```

---

## Task 7: Wire `provide_api()` in `app.rs`

Add a single `crate::api::provide_api()` call to the `App` component's startup so `use_api()` works at every callsite from now on.

**Files:**
- Modify: `crates/spa/src/app.rs`

- [ ] **Step 1: Edit `crates/spa/src/app.rs`, add `provide_api()` after `provide_locale()`**

The current top of `App` looks like:

```rust
#[component]
pub fn App() -> impl IntoView {
    provide_locale();
    if crate::demo::is_demo() {
        crate::demo::install_postmessage_nav();
    }
    let base: String = crate::demo::router_base();
    view! { … }
}
```

Change it to:

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

- [ ] **Step 2: Verify build still passes**

Run: `CARGO_TARGET_DIR=C:/Users/mayer/Documents/Grumps/target PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe check --target wasm32-unknown-unknown -p grumps-spa --message-format=short 2>&1 | tail -3`

Expected: `Finished`. `provide_api()` runs but no callsite reads the context yet; ApiClient still serves them.

- [ ] **Step 3: Commit**

```bash
git add crates/spa/src/app.rs
git commit -m "feat(spa): wire provide_api() at App startup

Pick LiveApi or DemoApi once based on is_demo() and stash the
Rc<dyn Api> in Leptos context for use_api() to read. Callsites
still use the legacy ApiClient — they migrate in the next task."
```

---

## Task 8: Migrate the 13 callsites to `use_api()`

Sweep every place that does `let api = ApiClient::new();` and replace with `let api = use_api();`. Every callsite also needs `Api` in scope so trait method dispatch works.

**Files (13 callers):**
- Modify: `crates/spa/src/components/sidebar.rs`
- Modify: `crates/spa/src/pages/calendar.rs`
- Modify: `crates/spa/src/pages/global_observability.rs`
- Modify: `crates/spa/src/pages/history.rs`
- Modify: `crates/spa/src/pages/memory.rs`
- Modify: `crates/spa/src/pages/note_editor.rs`
- Modify: `crates/spa/src/pages/notes.rs`
- Modify: `crates/spa/src/pages/observability.rs`
- Modify: `crates/spa/src/pages/overview.rs`
- Modify: `crates/spa/src/pages/scheduled.rs`
- Modify: `crates/spa/src/pages/settings.rs`
- Modify: `crates/spa/src/pages/todos.rs`

(Note: `pages/login.rs` may still call `ApiClient::new()` for the OTP flow — verify and migrate if so.)

- [ ] **Step 1: Run a sweep to find every callsite**

```bash
grep -rn "ApiClient::new\|use crate::api::ApiClient" crates/spa/src/
```

Expected output: ~13-14 lines, one per file in the list above.

- [ ] **Step 2: For each file, change the import and the construction**

Before:
```rust
use crate::api::ApiClient;
…
let api = ApiClient::new();
api.get_todos(&slug, "open").await
```

After:
```rust
use crate::api::{Api, use_api};
…
let api = use_api();
api.get_todos(&slug, "open").await
```

The `Api` import is required for trait-method dispatch on `Rc<dyn Api>`. The `use_api` import is the context fetcher. Replace `ApiClient::new()` with `use_api()` directly — `Rc<dyn Api>` is `Clone`, so `let api2 = api.clone();` keeps working unchanged.

Apply this pattern to every file in the list. The diff per file is exactly 2 lines (one import line, one constructor line).

- [ ] **Step 3: Verify the sweep is complete**

```bash
grep -rn "ApiClient::new\|use crate::api::ApiClient" crates/spa/src/
```

Expected output: empty (no remaining callers).

- [ ] **Step 4: Verify build still passes**

Run: `CARGO_TARGET_DIR=C:/Users/mayer/Documents/Grumps/target PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe check --target wasm32-unknown-unknown -p grumps-spa --message-format=short 2>&1 | tail -5`

Expected: `Finished`. Some pre-existing dead-code warnings remain; no errors.

- [ ] **Step 5: Commit**

```bash
git add crates/spa/src/components/sidebar.rs crates/spa/src/pages/
git commit -m "refactor(spa): migrate 13 callsites from ApiClient to use_api()

Every page/component that did 'let api = ApiClient::new();' now
fetches the Api handle from Leptos context via use_api(). The
Api trait is also imported so trait-method dispatch on the
Rc<dyn Api> resolves. Diff per file is two lines (import +
constructor); the .clone()-into-closures pattern keeps working
because Rc<dyn Api> is Clone."
```

---

## Task 9: Remove the legacy `ApiClient` struct + impl

With every callsite migrated, `ApiClient` has no users. Delete it.

**Files:**
- Modify: `crates/spa/src/api/mod.rs`

- [ ] **Step 1: Edit `crates/spa/src/api/mod.rs`, remove the legacy `ApiClient` block**

Find and delete the entire `pub struct ApiClient;` block plus its `impl ApiClient { … }` block (currently lines ~22-280 of `api/mod.rs` — everything from the `// API Client` comment through the closing `}` of the impl block).

After this edit, `api/mod.rs` should contain only:
- The `pub mod types; pub use types::*;` re-export
- The `use std::rc::Rc;` import
- The `Api` trait definition
- The `ApiHandle` typedef
- `provide_api` and `use_api`
- `pub mod live; pub mod demo;`

- [ ] **Step 2: Verify build still passes**

Run: `CARGO_TARGET_DIR=C:/Users/mayer/Documents/Grumps/target PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe check --target wasm32-unknown-unknown -p grumps-spa --message-format=short 2>&1 | tail -5`

Expected: `Finished`. If errors mention `ApiClient` not found, a callsite was missed in Task 8 — go back and fix.

- [ ] **Step 3: Run clippy as well**

Run: `CARGO_TARGET_DIR=C:/Users/mayer/Documents/Grumps/target PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe clippy --target wasm32-unknown-unknown -p grumps-spa --message-format=short 2>&1 | tail -10`

Expected: `Finished`, with the same 24-ish pre-existing warnings as before this PR. No new clippy lints from the refactor.

- [ ] **Step 4: Commit**

```bash
git add crates/spa/src/api/mod.rs
git commit -m "refactor(spa): delete legacy ApiClient struct + impl

All 13 callsites migrated to use_api(); ApiClient has zero
users. Removes the inline 'if is_demo()' guards along with the
struct itself — demo coverage is now type-checked via the
trait."
```

---

## Task 10: Manual demo iframe smoke test

Compile-check passing doesn't prove `DemoApi` returns shapes the components actually render. Load the demo iframe locally and click through the major pages.

**Files:** none (manual verification step).

- [ ] **Step 1: Build the SPA + landing bundle**

Per `CLAUDE.md`'s "Demo mode" section:

```bash
cd crates/spa
mkdir -p dist
tailwindcss -i ./input.css -o ./dist/styles.css --minify
MSYS_NO_PATHCONV=1 trunk build --release --public-url /demo/
cd ../..
node landing/build.mjs
```

Expected: `dist/demo/` contains the SPA bundle with the iframe `src` rewritten to `/demo/?lang=en`.

- [ ] **Step 2: Serve `dist/` and open the demo**

Use whichever static server you have (`python3 -m http.server`, `npx serve`, `simple-http-server`, `caddy file-server`, etc.) pointed at `dist/`. Open `http://localhost:<port>/?lang=en` in a browser, then click through the embedded iframe.

- [ ] **Step 3: Click through the demo iframe pages**

In the browser, on the demo iframe (or `/demo/?lang=en` directly), confirm each page renders without console errors:

- Dashboard / Overview — workspace list, status counts, pinned memory
- Todos — list shows seed todos, filter switches, "+ Add" button doesn't crash
- Notes — list shows seed notes, clicking one opens the editor
- Calendar (month + week + agenda views) — events render
- Memory — pinned + non-pinned items show
- Scheduled — list shows seed actions
- Settings — sections render, language switcher works
- Sidebar — workspace switcher, language switcher

Open the browser console: should be empty of red errors. Yellow warnings from the framework are acceptable.

- [ ] **Step 4: Note any broken pages**

If any page rendered empty, broken, or threw a console error, the corresponding `DemoApi` method is returning the wrong shape — fix and recommit before proceeding to Task 11.

If everything renders, no commit is needed for this task. The smoke test is a gate, not a code change.

---

## Task 11: Update audit follow-ups doc

Flip #11 from DEFERRED to ✅ in the followups index.

**Files:**
- Modify: `docs/superpowers/2026-04-26-audit-followups.md`

- [ ] **Step 1: Edit the doc, find the #11 section**

Replace:

```markdown
### #11 — Demo-mode guards scattered through `crates/spa/src/api.rs`
**DEFERRED**. The trait-based refactor (`trait Api` with
`async-trait`, `DemoApi` + `LiveApi` impls, `Rc<dyn Api>` factory)
touches 30+ method signatures and 13 callsites. Worth doing on its
own PR with focused review; out of scope for this batch.
```

with:

```markdown
### #11 — Demo-mode guards scattered through `crates/spa/src/api.rs`
✅ Done in 2026-04-27 audit-medium-2 pass. `crates/spa/src/api.rs`
split into `api/{mod,types,live,demo}.rs`. `trait Api` covers all
35 client methods; `LiveApi` does HTTP, `DemoApi` delegates to
`crate::demo::*` seed functions and returns `Err` for auth.
`provide_api()` runs once on `App` startup; 13 callsites use
`use_api()` to fetch `Rc<dyn Api>` from context. Forgetting a
demo branch on a future method is now a compile error.

Spec: `docs/superpowers/specs/2026-04-27-api-trait-refactor-design.md`.
Plan: `docs/superpowers/plans/2026-04-27-api-trait-refactor.md`.
```

- [ ] **Step 2: Commit**

```bash
git add docs/superpowers/2026-04-26-audit-followups.md
git commit -m "docs: mark audit follow-up #11 as done

The Api trait refactor landed. crates/spa/src/api.rs no longer
has scattered 'if is_demo()' guards; demo coverage is enforced
by the type system via trait Api with LiveApi + DemoApi impls."
```

---

## Final state

After all 11 tasks complete:

```
crates/spa/src/api/
  mod.rs       Trait Api (35 methods), ApiHandle = Rc<dyn Api>,
               provide_api(), use_api(), pub mod types/live/demo,
               pub use types::*.
  types.rs     All response/request DTOs.
  live.rs      LiveApi + #[async_trait(?Send)] impl Api with HTTP
               bodies + private get/post/put/patch/delete helpers.
  demo.rs      DemoApi + #[async_trait(?Send)] impl Api with seed
               delegations + Err for auth.

crates/spa/src/api.rs        DELETED.

crates/spa/src/app.rs        Calls crate::api::provide_api() at startup.

crates/spa/src/{components,pages}/*.rs     13 callsites use use_api().
```

Commits on the branch (in order):

1. `build(spa): add async-trait dependency for Api trait refactor`
2. `refactor(spa): move api.rs into api/ module directory`
3. `refactor(spa): extract api response types into types.rs`
4. `feat(spa): introduce Api trait, ApiHandle, provide_api/use_api`
5. `feat(spa): populate LiveApi with HTTP bodies`
6. `feat(spa): populate DemoApi with seed-data delegations`
7. `feat(spa): wire provide_api() at App startup`
8. `refactor(spa): migrate 13 callsites from ApiClient to use_api()`
9. `refactor(spa): delete legacy ApiClient struct + impl`
10. (no commit — manual smoke test)
11. `docs: mark audit follow-up #11 as done`
