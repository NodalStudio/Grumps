# Routes-layer extractors, shared DTOs & declarative validation — Design

**Date:** 2026-06-03
**Status:** Approved (design phase)
**Scope:** `crates/worker` (routes + middleware), `crates/core` (shared DTOs), `crates/spa` (API trait)

## Problem

Every workspace-scoped handler in the worker repeats the same ~25-line preamble:

```rust
let claims = match middleware::verify_session(&req, &ctx.env).await { ... };
let ws = match resolve_workspace(&ctx).await { ... };
if !claims.workspaces.contains(&ws.slug) { return 403 }
let body: CreateTodo = match req.json().await { ... };
if body.title.is_empty() { return 400 }
if body.title.chars().count() > 500 { return 400 }
if let Some(p) = body.priority { if !(1..=3).contains(&p) { return 400 } }
```

Three distinct problems compound here:

1. **Auth/workspace guard duplicated** at the top of ~15 handlers across 8 route files. `resolve_workspace` itself is copy-pasted verbatim into 8 files (`todos`, `notes`, `memory`, `scheduled`, `events`, `calendar`, `observability`, `workspace_api`).
2. **Validation is hand-rolled** per field, with hard-coded English error strings (`"title required"`, `"title must be ≤ 500 characters"`) — a violation of the project's hard i18n rule (no user-facing text in source).
3. **No typed wire contract.** The SPA `Api` trait uses positional args `create_todo(slug, title, priority)` that can't even express the `tags`/`assigned_to`/`deadline` the endpoint already accepts. The worker's request DTO is a private struct, unshareable.

The root cause is the absence of an extraction/validation layer. `worker::Error` carries no `(status, code)`, so `?` can't propagate a structured HTTP error with CORS — forcing every branch to call `error_with_cors` by hand.

## Goals

- A workspace-route handler becomes pure business logic: `(req, ctx, P, body)`, no auth/validation/parsing preamble.
- Access policy reads off the route table in `lib.rs`, not hidden in function bodies.
- Zero hard-coded English in route `.rs` files — every error is an i18n code.
- One copy of `resolve_workspace`.
- Typed request DTOs shared SPA↔worker; the SPA can send every field the endpoint accepts.
- Adding an access level = one trait impl; adding a field = one DTO line.

## Approach

Borrow axum's load-bearing ideas at a minimal dosage ("axum-lite"), without its `Handler` trait + macro-generated tuple machinery:

1. **Split parts-extraction (no body) from body-extraction (consumes body, last).** Auth/workspace are parts; the JSON DTO is the body. Only one thing consumes the body.
2. **One rejection type that converts to a response.** A single conversion point, not scattered `match` arms.
3. **Each extractor single-responsibility, testable in isolation.** Compose, don't fuse.

The simplification vs full axum: a *single* part type per route (e.g. `Member` bundling claims+ws) rather than arbitrary tuples of parts. This avoids tuple-impl machinery while keeping composition.

### Approaches considered

- **A — Fixed combinator matrix.** One named wrapper per case (`member`, `member_json`, `admin`, `admin_json`, `authed`, `authed_json`). Simplest, no trait machinery. Cost: N×M variants; each new access level multiplies wrappers. **Kept as the fallback** if the trait approach fails to compile under workers-rs Router bounds.
- **B — `FromParts` trait + 2 combinators (axum-lite). CHOSEN.** Each access level written once as a trait impl; two generic combinators compose any of them. Adding a level = one impl. More composable, slightly more advanced Rust.
- **C — Full extractor system (parts + body tuples, generic over arbitrary combinations).** Closest to axum, heaviest type machinery. Over-engineered for ~15 handlers. Rejected.

## Architecture

Four layers, each one responsibility.

### 1. `ApiError` (in `middleware.rs`)

Generalizes the existing `AuthError` (which already exposes `.status()` / `.code()`).

```rust
pub struct ApiError { status: u16, code: Cow<'static, str> }

impl ApiError {
    pub fn new(status: u16, code: impl Into<Cow<'static, str>>) -> Self;
    pub fn bad_request(code: impl Into<Cow<'static, str>>) -> Self;   // 400
    pub fn forbidden(code: impl Into<Cow<'static, str>>) -> Self;     // 403
    pub fn not_found(code: impl Into<Cow<'static, str>>) -> Self;     // 404
    pub fn into_response(self, req: &Request) -> Result<Response>;     // code doubles as detail → no English in source
}

impl From<AuthError> for ApiError { /* reuse .status()/.code() */ }
```

`code` is `Cow<'static, str>` (not `&'static str`) because `validator` error codes are `Cow`.

### 2. The `FromParts` trait

"How to extract this from the request without touching the body."

```rust
#[async_trait(?Send)]
pub trait FromParts: Sized {
    async fn from_parts(req: &Request, ctx: &RouteContext<()>) -> Result<Self, ApiError>;
}
```

Three impls = the three access levels, each written once:

- `Session(pub Claims)` — `verify_session` only.
- `Member { pub claims: Claims, pub ws: WorkspaceMetaRow }` — session + `resolve_workspace` + `claims.workspaces.contains(&ws.slug)`.
- `Admin { pub claims: Claims, pub ws: WorkspaceMetaRow }` — `Member` + `role == 'admin'` (`is_workspace_admin`).

Adding a level later (e.g. super-admin) = one new `impl FromParts`; nothing else changes.

### 3. Two public combinators

The only "glue", generic:

```rust
// parts only, no body
pub fn route<P, H, F>(h: H) -> impl Fn(Request, RouteContext<()>) -> Pin<Box<dyn Future<Output = Result<Response>>>>
where
    P: FromParts + 'static,
    H: Fn(Request, RouteContext<()>, P) -> F + Clone + 'static,
    F: Future<Output = Result<Response>> + 'static;

// parts + validated JSON body (body consumed last)
pub fn route_json<P, B, H, F>(h: H) -> impl Fn(Request, RouteContext<()>) -> Pin<Box<dyn Future<Output = Result<Response>>>>
where
    P: FromParts + 'static,
    B: serde::de::DeserializeOwned + validator::Validate + 'static,
    H: Fn(Request, RouteContext<()>, P, B) -> F + Clone + 'static,
    F: Future<Output = Result<Response>> + 'static;
```

`route_json` flow: extract `P` → `req.json::<B>()` (400 `bad_request` if malformed) → `B::validate()` (400 + first validation code if invalid) → call handler. One thing consumes the body, and it's last — axum's rule.

Async closures aren't first-class in Rust → the combinators return `Pin<Box<dyn Future>>` (one `Box::pin` per request, negligible vs a D1 call).

### 4. Call site (`lib.rs`)

`P` and `B` are inferred from the handler's signature → no turbofish:

```rust
.post_async  ("/api/w/:slug/todos",     route_json(todos::create_todo))
.get_async   ("/api/w/:slug/todos",     route    (todos::list_todos))
.patch_async ("/api/w/:slug/todos/:id", route_json(todos::update_todo))
.delete_async("/api/w/:slug/todos/:id", route    (todos::delete_todo))
```

Handler = pure business logic:

```rust
pub async fn create_todo(req: Request, ctx: RouteContext<()>, m: Member, body: CreateTodoRequest)
    -> Result<Response>
{
    // m.claims, m.ws, body — all validated. Insert.
}
```

Access policy reads off the route table (`route` vs `route_json`, `Member` vs `Admin` via the handler's part type).

## Shared DTOs & validation

New module `crates/core/src/dto.rs`. `serde` always; `validator` behind a `validation` feature (worker only, kept out of the SPA bundle).

```rust
#[derive(serde::Serialize, serde::Deserialize, Default)]
#[cfg_attr(feature = "validation", derive(validator::Validate))]
pub struct CreateTodoRequest {
    #[cfg_attr(feature = "validation",
        validate(length(min = 1, max = 500, code = "todo.title_invalid")))]
    pub title: String,

    #[cfg_attr(feature = "validation",
        validate(range(min = 1, max = 3, code = "todo.priority_invalid")))]
    pub priority: Option<i32>,

    pub tags: Option<Vec<String>>,
    pub assigned_to: Option<String>,
    pub assigned_name: Option<String>,
    pub deadline: Option<String>,
}
```

The `cfg_attr` gates both the derive and the `validate(...)` field attributes, so with the feature off (SPA) `validator` is not compiled at all.

### Cargo wiring

- `core/Cargo.toml`: `validator = { version = "0.18", optional = true }` + `[features] validation = ["dep:validator"]`
- `worker/Cargo.toml`: `grumps-core = { path = "../core", features = ["validation"] }`
- `spa/Cargo.toml`: add `grumps-core = { path = "../core" }` (no feature)

### SPA side

The `Api` trait moves from positional to typed DTO:

```rust
// before
async fn create_todo(&self, slug: &str, title: &str, priority: i32) -> Result<TodoItem, String>;
// after
async fn create_todo(&self, slug: &str, req: CreateTodoRequest) -> Result<TodoItem, String>;
```

`slug` stays a separate path param. `live.rs` serializes the DTO instead of the hand-written `json!({...})`; `demo.rs` reads typed fields.

### DTO location: shared vs worker-local (refinement during implementation)

Not every request DTO belongs in `core`. Two cases:

- **Shared in `core::dto`** — DTOs for SPA APIs that were *positional* (`create_todo(slug, title, priority)`), where a typed struct is a real ergonomic win on both sides: `CreateTodoRequest`, `CreateNoteRequest`, `UpdateNoteRequest`.
- **Worker-local** — DTOs that reference worker-only domain types (e.g. memory's `CreateBody` uses `grumps_memory::MemoryKind`). Moving these to `core` would force `core` to depend on `grumps-memory`, risking a dependency cycle and bloating the SPA bundle. They stay in their route module but still `#[derive(validator::Validate)]` and flow through `route_json` — same guard/validation benefit. The SPA continues to send these as `serde_json::Value` (unchanged), since those `Api` methods were already `Value`-based, not positional.

The rule: share a DTO only when both sides gain from the shared type *and* it pulls no worker-only deps into `core`.

### Form vs business validation

`validator` covers form only (length, range, date regex). DB-touching checks (e.g. "is `assigned_to` a member?") stay in the handler, after extraction. We do not try to push everything into the DTO.

## Migration plan

Endpoint by endpoint, `todos.rs` first as the reference model, TDD throughout. No big-bang; the app compiles and tests pass at every step. Each step is one atomic gitmoji commit.

1. **Foundations (no handler touched yet).** `ApiError` + `From<AuthError>` + `into_response`; `FromParts` trait + `Session`/`Member`/`Admin` impls; `route`/`route_json` combinators; lift `resolve_workspace` into `middleware.rs` (kills the 8 copies). **Test:** one rejection test per case (401 unauthenticated, 403 non-member, 403 non-admin, 404 unknown workspace, 400 malformed JSON, 400 validation). This is the critical auth layer — covered before any migration. **Compile a throwaway wasm worker build here** to de-risk async-trait/Router bounds before going further.
2. **`core::dto`.** `CreateTodoRequest` + `validation` feature + Cargo wiring of the 3 crates. **Test:** serde round-trip + `validate()` accept/reject.
3. **Migrate `todos.rs`** (4 routes) onto the combinators + DTO. SPA side: `Api` trait, `live.rs`, `demo.rs`. **Test:** rejections flow through the real todos handlers; SPA compiles to wasm. **Checkpoint:** show this full diff as the model; approve the shape before propagating.
4. **Propagate** to the other 7 route files (`notes`, `memory`, `scheduled`, `events`, `calendar`, `observability`, `workspace_api`), one DTO per surface as needed. One step = one file, independently verifiable.
5. **Cleanup.** Remove the private `struct CreateTodo` DTOs in routes; replace hard-coded-English `error_with_cors` calls with codes; confirm no hard-coded English remains in route `.rs` files.

**Final verification:** `cargo test --target x86_64-pc-windows-msvc` (host) **and** wasm builds of both worker and SPA (`trunk build`) to confirm `validator`/`async-trait` compile on both targets.

## Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| `FromParts` (async-trait `?Send`) won't compile under workers-rs Router bounds | Medium | Step 1 is a throwaway wasm prototype before any handler. If it fails, fall back to fixed matrix (approach A) — same DTO, same `ApiError`, only the glue changes. |
| `P`/`B` inference fails → turbofish at call site | Low | Acceptable fallback; still one line. Verified in step 1. |
| `validator` or its macros incompatible with wasm | Low | Gated behind `validation` (worker only); worker already compiles proc-macros to wasm. Tested step 2. |
| SPA bundle bloat from `core` | Low | `validation` off on SPA; `core` only adds serde/chrono already present. |
| Auth regression during migration | Medium | Rejection test suite written in step 1, before touching any handler. |

## Success criteria

1. A workspace-route handler is pure business logic: `(req, ctx, P, body)`, zero auth/validation/parsing preamble.
2. Access policy reads off `lib.rs` (`route`/`route_json` + part type).
3. Zero hard-coded English in route `.rs` files — all errors are i18n codes.
4. `resolve_workspace` exists in exactly one place.
5. Request DTOs typed and shared SPA↔worker; the SPA can send every field the endpoint accepts.
6. `cargo test` (host) and wasm builds (worker + SPA) green.
7. Adding an access level = one `impl FromParts`; adding a field = one DTO line.

## Implementation outcome (2026-06-03)

Delivered on branch `feat/routes-layer-extractors`:

- `extract.rs`: `ApiError` (+ `From<AuthError>`), `FromParts` trait, `Session`/`Member`/`Admin`, `route`/`route_json`. Proven to compile under the workers-rs Router bounds on `wasm32-unknown-unknown` — approach B held, no fallback to the fixed matrix needed.
- All eight workspace route files migrated to guards; `resolve_workspace` collapsed from 8 copies to 1 (in `extract.rs`). Net ≈ −1000 lines across the route layer.
- Shared `core::dto`: `CreateTodoRequest`, `UpdateTodoRequest`, `CreateNoteRequest`, `UpdateNoteRequest` (validator gated behind `validation`). Memory/events/scheduled keep worker-local DTOs (worker-only domain deps). SPA todos + notes use the shared types; memory/events/scheduled still post `serde_json::Value` (wire-compatible).
- Dead auth helpers removed (`check_workspace_access`, `is_workspace_admin`).

**Realized test strategy.** Worker handlers take `worker::Request`, which can't be constructed in host unit tests, so the guard *rejection paths* are covered by the existing (ignored) integration suite that hits a live `wrangler dev`, not by host units. What IS host-tested: DTO validation rules (6 tests in `core::dto`) and the rejection *contract* — `AuthError → (status, code)` mapping and `first_validation_code` (3 tests in `extract.rs`). Both green via `cargo test --target x86_64-pc-windows-msvc`.

**Follow-up — i18n dictionary keys.** The migration introduced error codes (`todo.title_invalid`, `note.not_found`, `event.range_invalid`, `timezone.unsupported`, `auth.not_admin`, …) that the SPA renders via `tr()`. These keys must be added to the 14 locale dictionaries (English first, then the translate script). Until then they degrade to the literal key per the `t_plural`/`tr` fallback chain — graceful but visible.

## Out of scope (v1) — noted as future work

- Structured params to interpolate validation messages (`{"max": 500}` for "max 500 chars").
- Generic business-rule validation.
- axum-style tuple extractors (approach C).
