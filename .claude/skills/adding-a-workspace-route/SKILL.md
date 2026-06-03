---
name: adding-a-workspace-route
description: Use when adding, editing, or reviewing an HTTP endpoint in the Grumps worker (crates/worker/src/routes/) — the axum-lite extractor pattern (Session/Member/Admin guards, route()/route_json() combinators, ApiError, shared core::dto request DTOs with validator). Apply instead of re-writing per-handler verify_session/resolve_workspace/validation boilerplate.
---

# Adding a workspace HTTP route

The Grumps worker uses an "axum-lite" extraction layer so handlers are pure
business logic. Auth, workspace resolution, membership/role checks, body
deserialization, and shape validation all happen in reusable guards/combinators
— never inline in the handler.

Source of truth: `crates/worker/src/extract.rs`. Reference model:
`crates/worker/src/routes/todos.rs`. Rationale:
`docs/superpowers/specs/2026-06-03-routes-layer-extractors-design.md`.

## The three guards (`FromParts` impls in `extract.rs`)

| Guard | What it proves | Use for |
|---|---|---|
| `Session(pub Claims)` | valid auth session | user-scoped routes (`/api/me`, `/api/workspaces`) |
| `Member { claims, ws }` | session + `:slug` workspace resolved + caller is a member | the default for `/api/w/:slug/...` |
| `Admin { claims, ws }` | `Member` + admin role (super-admin overrides) | settings/destructive ops |

If a route needs a *stricter* gate than `Admin` (e.g. super-admin only), take
`Member` and keep the extra check inline returning an `ApiError` — see
`routes/observability.rs`.

## Steps to add an endpoint

1. **Write the handler** in the relevant `routes/*.rs`. It receives the guard
   (and the body DTO if it has one) already validated:
   ```rust
   pub async fn create_todo(
       req: Request,
       ctx: RouteContext<()>,
       m: Member,                 // or Session / Admin
       body: CreateTodoRequest,   // omit for GET/DELETE
   ) -> Result<Response> {
       // m.claims, m.ws, body — all valid. Pure business logic.
   }
   ```
   No `verify_session`, no `resolve_workspace`, no membership `if`, no
   `req.json()` match, no per-field validation. Path params other than `:slug`
   (e.g. `:id`) are read from `ctx.param("id")` in the handler.

2. **Define the request DTO** (POST/PUT/PATCH only):
   - Shared with the SPA → `crates/core/src/dto.rs`:
     ```rust
     #[derive(serde::Serialize, serde::Deserialize, Default, Debug, Clone)]
     #[cfg_attr(feature = "validation", derive(validator::Validate))]
     pub struct CreateTodoRequest {
         #[cfg_attr(feature = "validation",
             validate(length(min = 1, max = 500, code = "todo.title_invalid")))]
         pub title: String,
         // ...
     }
     ```
     Both the `derive(Validate)` and the `validate(...)` attrs are gated behind
     the `validation` feature so `validator` never enters the SPA wasm bundle.
   - References a worker-only type (e.g. `grumps_memory::MemoryKind`) → keep it
     **local** to the route module with `#[derive(Deserialize, Validate)]`. Do
     NOT move it to `core` (would create a dependency cycle). See `routes/memory.rs`.
   - Only **shape** rules go in `#[validate]`. Cross-field (`ends >= starts`),
     time (`trigger_at` in future), or DB checks stay in the handler.

3. **Register in `crates/worker/src/lib.rs`** with a combinator — never the bare
   handler:
   ```rust
   .get_async   ("/api/w/:slug/todos",     extract::route(routes::todos::list_todos))
   .post_async  ("/api/w/:slug/todos",     extract::route_json(routes::todos::create_todo))
   .delete_async("/api/w/:slug/todos/:id", extract::route(routes::todos::delete_todo))
   ```
   `route` = guard only; `route_json` = guard + validated body. The guard and
   body types are inferred from the handler signature.

4. **Errors** use `ApiError`, returned with `.into_response(&req)`:
   ```rust
   return ApiError::not_found("todo.not_found").into_response(&req);
   // helpers: bad_request(403→) forbidden, not_found, internal(), new(status, code)
   ```
   The `code` is an **i18n key**, never English prose. Add new keys to
   `crates/i18n/locales/*.json` (English first). Auth/validation rejections are
   produced by the guards/`route_json` automatically — you only emit `ApiError`
   for handler-level cases (404 not-found, domain checks).

5. **SPA side** (if the endpoint is called from the SPA): add the method to the
   `Api` trait (`crates/spa/src/api/mod.rs`) and both impls (`live.rs`, `demo.rs`).
   For shared DTOs, the method takes the DTO by value
   (`create_todo(&self, slug, req: CreateTodoRequest)`); `live.rs` serializes it
   directly (`self.post(path, &req)`).

## Checklist before finishing

- [ ] Handler has no auth/workspace/parse/validate preamble — just the guard params.
- [ ] Registered with `extract::route` / `extract::route_json`, not the bare fn.
- [ ] No English strings in `error_with_cors` / `Response::error` — use `ApiError` + i18n code.
- [ ] New i18n codes added to all 14 locale dictionaries (or noted as follow-up).
- [ ] `cargo check -p grumps-worker --target wasm32-unknown-unknown` is clean.
- [ ] If SPA-facing: SPA compiles for wasm too.
