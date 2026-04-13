# Phase 2: Web Workspace — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the web workspace — a Leptos CSR SPA deployed to CF Pages with OTP auth, all workspace pages (dashboard, todos, notes, files, history, settings), and the corresponding Worker API routes with JWT auth and CORS.

**Architecture:** Two deployments: (1) the existing Worker gets API routes (CORS, JWT middleware, CRUD endpoints) and OTP auth, (2) a new Leptos CSR SPA compiled to WASM via Trunk, deployed to CF Pages. The SPA calls the Worker API via `fetch()`. JWT is stored in memory (not localStorage). The validated design system from `workspace.html` is translated to Leptos components + Tailwind CSS.

**Tech Stack:** Leptos 0.7+ (CSR), leptos_router, gloo-net (fetch), Trunk, Tailwind CSS, jsonwebtoken (Worker)

---

## File Structure (new/modified files only)

```
grumps/
├── crates/
│   ├── worker/src/
│   │   ├── lib.rs                     # Add API routes
│   │   ├── middleware.rs              # NEW: JWT verification + CORS
│   │   └── routes/
│   │       ├── mod.rs                 # Add new route modules
│   │       ├── auth.rs               # NEW: OTP send + verify
│   │       ├── todos.rs              # NEW: CRUD API
│   │       ├── notes.rs              # NEW: CRUD API
│   │       ├── workspace_api.rs      # NEW: settings, members, history
│   │       ├── webhook.rs            # existing
│   │       └── health.rs             # existing
│   └── spa/                           # NEW: Leptos CSR app
│       ├── Cargo.toml
│       ├── Trunk.toml
│       ├── index.html                 # HTML shell
│       ├── tailwind.config.js
│       ├── input.css                  # Tailwind + design system
│       └── src/
│           ├── main.rs                # mount_to_body
│           ├── app.rs                 # Router + top-level App
│           ├── api.rs                 # fetch() helpers + JWT state
│           ├── auth.rs                # Auth state + guard
│           ├── pages/
│           │   ├── mod.rs
│           │   ├── login.rs           # OTP login flow
│           │   ├── dashboard.rs       # My workspaces
│           │   ├── workspace.rs       # Layout shell (sidebar + outlet)
│           │   ├── overview.rs        # Workspace overview
│           │   ├── todos.rs           # Todo list + kanban
│           │   ├── notes.rs           # Note list
│           │   ├── note_editor.rs     # Note read/edit
│           │   ├── files.rs           # File grid
│           │   ├── history.rs         # Activity log
│           │   └── settings.rs        # Workspace settings
│           └── components/
│               ├── mod.rs
│               ├── todo_card.rs       # Single todo item
│               ├── todo_filters.rs    # Filter bar
│               ├── kanban.rs          # Kanban board
│               ├── note_card.rs       # Note card
│               ├── file_card.rs       # File card
│               ├── sidebar.rs         # Sidebar nav
│               ├── header.rs          # Page header
│               ├── toast.rs           # Toast notifications
│               └── empty_state.rs     # Empty states with personality
```

---

## Task 1: Worker — CORS Middleware + JWT Auth

**Files:**
- Create: `crates/worker/src/middleware.rs`
- Modify: `crates/worker/src/lib.rs` (add CORS preflight)

JWT verification for API routes. CORS headers on all API responses. OTP auth comes in Task 2.

- [ ] **Step 1: Create middleware.rs**

```rust
// crates/worker/src/middleware.rs
use worker::*;

const ALLOWED_ORIGIN: &str = "https://grumps.io";
const DEV_ORIGIN: &str = "http://localhost:8080";

/// Add CORS headers to a response.
pub fn cors_headers(resp: &mut Response, origin: Option<&str>) -> Result<()> {
    let allowed = match origin {
        Some(o) if o == ALLOWED_ORIGIN || o.starts_with("http://localhost") => o,
        _ => DEV_ORIGIN,
    };
    let headers = resp.headers_mut();
    headers.set("Access-Control-Allow-Origin", allowed)?;
    headers.set("Access-Control-Allow-Methods", "GET, POST, PUT, PATCH, DELETE, OPTIONS")?;
    headers.set("Access-Control-Allow-Headers", "Content-Type, Authorization")?;
    headers.set("Access-Control-Max-Age", "86400")?;
    Ok(())
}

/// Handle CORS preflight (OPTIONS request).
pub fn cors_preflight(req: &Request) -> Result<Response> {
    let origin = req.headers().get("Origin")?.unwrap_or_default();
    let mut resp = Response::empty()?;
    resp.headers_mut().set("Content-Length", "0")?;
    cors_headers(&mut resp, Some(&origin))?;
    Ok(resp.with_status(204))
}

/// Extract and verify JWT from Authorization header.
/// Returns the claims if valid.
pub fn verify_jwt(req: &Request, jwt_secret: &str) -> std::result::Result<JwtClaims, String> {
    let auth = req.headers().get("Authorization")
        .map_err(|_| "missing auth header".to_string())?
        .ok_or_else(|| "missing Authorization header".to_string())?;

    let token = auth.strip_prefix("Bearer ")
        .ok_or_else(|| "invalid auth format".to_string())?;

    // Decode JWT (using jsonwebtoken crate)
    let key = jsonwebtoken::DecodingKey::from_secret(jwt_secret.as_bytes());
    let validation = jsonwebtoken::Validation::default();

    let data = jsonwebtoken::decode::<JwtClaims>(token, &key, &validation)
        .map_err(|e| format!("invalid token: {}", e))?;

    Ok(data.claims)
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct JwtClaims {
    pub sub: String,           // user_id
    pub phone: String,
    pub workspaces: Vec<String>, // workspace slugs
    pub exp: usize,
}

/// Create a signed JWT.
pub fn create_jwt(user_id: &str, phone: &str, workspaces: Vec<String>, jwt_secret: &str) -> std::result::Result<String, String> {
    let exp = chrono::Utc::now().timestamp() as usize + 7 * 24 * 3600; // 7 days
    let claims = JwtClaims {
        sub: user_id.to_string(),
        phone: phone.to_string(),
        workspaces,
        exp,
    };
    let key = jsonwebtoken::EncodingKey::from_secret(jwt_secret.as_bytes());
    jsonwebtoken::encode(&jsonwebtoken::Header::default(), &claims, &key)
        .map_err(|e| format!("jwt encode error: {}", e))
}
```

- [ ] **Step 2: Add jsonwebtoken to worker Cargo.toml**

Add: `jsonwebtoken = "9"` to `[dependencies]`

- [ ] **Step 3: Update lib.rs with CORS preflight handler**

Add before the router: an OPTIONS catch-all that returns CORS headers.

- [ ] **Step 4: Verify compilation, commit**

---

## Task 2: Worker — OTP Auth Routes

**Files:**
- Create: `crates/worker/src/routes/auth.rs`

POST `/auth/otp` — send OTP code via WhatsApp
POST `/auth/verify` — verify code, return JWT

- [ ] **Step 1: Implement auth routes**

```rust
// crates/worker/src/routes/auth.rs
// POST /auth/otp { phone, workspace_slug }
//   → generate 6-digit code, store in KV (TTL 5min), send via WhatsApp
// POST /auth/verify { phone, code, workspace_slug }
//   → verify code from KV, lookup user workspaces, issue JWT
```

Uses KV for OTP storage, WhatsApp adapter to send the code, middleware::create_jwt for token.

---

## Task 3: Worker — Todo CRUD API

**Files:**
- Create: `crates/worker/src/routes/todos.rs`

```
GET    /api/w/:slug/todos          # List (filters via query params)
POST   /api/w/:slug/todos          # Create
PATCH  /api/w/:slug/todos/:id      # Update
DELETE /api/w/:slug/todos/:id      # Delete (soft)
```

All routes: verify JWT, check workspace membership, use WorkspaceDb, add CORS.

---

## Task 4: Worker — Notes + Workspace API

**Files:**
- Create: `crates/worker/src/routes/notes.rs`
- Create: `crates/worker/src/routes/workspace_api.rs`

Notes: GET list, POST create, GET by id, PUT update, DELETE
Workspace: GET history, GET members, GET settings, PUT settings

---

## Task 5: Worker — Wire all API routes

**Files:**
- Modify: `crates/worker/src/lib.rs`
- Modify: `crates/worker/src/routes/mod.rs`

Add all routes to the router. Verify compilation.

---

## Task 6: SPA — Leptos CSR Scaffold

**Files:**
- Create: `crates/spa/Cargo.toml`
- Create: `crates/spa/Trunk.toml`
- Create: `crates/spa/index.html`
- Create: `crates/spa/src/main.rs`
- Create: `crates/spa/src/app.rs`
- Create: `crates/spa/input.css` + `tailwind.config.js`

Set up the Leptos CSR app with Trunk, Tailwind CSS using the design system from workspace.html (Bitter font, cream/brick/teal palette, 2px borders, grain texture).

---

## Task 7: SPA — API Client + Auth State

**Files:**
- Create: `crates/spa/src/api.rs`
- Create: `crates/spa/src/auth.rs`

`api.rs`: fetch() wrapper that adds JWT header, handles errors, provides typed API calls (get_todos, create_todo, etc.).

`auth.rs`: AuthState context (logged in/out, JWT, user info), auth guard for protected routes, login/logout actions.

---

## Task 8: SPA — Login Page (OTP Flow)

**Files:**
- Create: `crates/spa/src/pages/login.rs`

Phone input → send OTP → enter 6-digit code → verify → redirect to dashboard. Uses the design from the HTML prototype (centered card, Bitter heading, OTP digit inputs).

---

## Task 9: SPA — Sidebar + Workspace Layout

**Files:**
- Create: `crates/spa/src/components/sidebar.rs`
- Create: `crates/spa/src/components/header.rs`
- Create: `crates/spa/src/pages/workspace.rs`

The workspace shell: sidebar with nav items + workspace selector, page header, main content area with `<Outlet/>`. Translates the sidebar from workspace.html.

---

## Task 10: SPA — Dashboard Page

**Files:**
- Create: `crates/spa/src/pages/dashboard.rs`

Lists user's workspaces as cards. Fetches from GET /api/workspaces.

---

## Task 11: SPA — Todos Page (List + Kanban)

**Files:**
- Create: `crates/spa/src/pages/todos.rs`
- Create: `crates/spa/src/components/todo_card.rs`
- Create: `crates/spa/src/components/todo_filters.rs`
- Create: `crates/spa/src/components/kanban.rs`

Two views: list and kanban. Filter bar (all/open/done/mine/assignee/tag). Inline todo creation. Checkbox toggle with optimistic updates.

---

## Task 12: SPA — Notes Page + Editor

**Files:**
- Create: `crates/spa/src/pages/notes.rs`
- Create: `crates/spa/src/pages/note_editor.rs`
- Create: `crates/spa/src/components/note_card.rs`

Note grid, note read view with markdown rendering, note editor (markdown textarea + preview).

---

## Task 13: SPA — Files, History, Settings Pages

**Files:**
- Create: `crates/spa/src/pages/files.rs`
- Create: `crates/spa/src/pages/history.rs`
- Create: `crates/spa/src/pages/settings.rs`
- Create: `crates/spa/src/pages/overview.rs`

Files: grid with upload zone (R2 upload in Phase 2). History: activity log. Settings: workspace config with toggles. Overview: stats + recent activity.

---

## Task 14: SPA — Toast + Empty States + Polish

**Files:**
- Create: `crates/spa/src/components/toast.rs`
- Create: `crates/spa/src/components/empty_state.rs`

Toast notification system. Empty states with personality. Final polish pass.

---

## Task 15: Deploy SPA to CF Pages

- Build: `cd crates/spa && trunk build --release`
- Deploy: `wrangler pages deploy crates/spa/dist/`
- Configure custom domain

---

## Architecture

```
  Browser (SPA)                    CF Worker (API)
  ┌──────────┐                   ┌─────────────────┐
  │ Leptos   │ ── fetch() ──→    │ JWT verify      │
  │ CSR/WASM │ ← JSON ────      │ CORS headers    │
  │ CF Pages │                   │ Routes:         │
  └──────────┘                   │  /auth/otp      │
                                 │  /auth/verify   │
                                 │  /api/w/:slug/* │
                                 │  /webhook/wa    │
                                 └────────┬────────┘
                                          │
                                  Index DB (native)
                                  Workspace DBs (REST)
                                  KV (OTP + dedup)
```
