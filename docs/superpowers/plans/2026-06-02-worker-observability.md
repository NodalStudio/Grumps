# Worker Observability & Error Identification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every request the worker receives visible and every error identifiable, by adding a per-request correlation id, structured leveled logs (incl. request-in/out and errors), a WASM panic hook, and Cloudflare Workers Logs retention.

**Architecture:** Centralise all logging in `crates/worker/src/observability.rs` as small *pure* helpers (correlation-id derivation, level/status mapping, field building) that are unit-testable on the host target, plus thin side-effecting wrappers around `worker::console_log!`. Wire request-in/out + error logging into the single fetch entry point (`lib.rs`), thread the same `cf-ray`-derived id through the Telegram path, and turn on Workers Logs in `wrangler.toml` so logs are retained and queryable in the Cloudflare dashboard (not just live `wrangler tail`).

**Tech Stack:** Rust + `worker` 0.8.3 (Cloudflare Workers), `serde_json`, `console_error_panic_hook`, `wrangler` 4.x. Host tests build with `--target x86_64-pc-windows-msvc` (see CLAUDE.md).

---

## Why "see all received requests" works

Cloudflare gives three layers; this plan uses the first two:

1. **`wrangler tail`** — live stream of every request while connected (`npx wrangler tail --format pretty`, filters `--status error` / `--method POST`). Ephemeral.
2. **Workers Logs** (`[observability]` in `wrangler.toml`) — Cloudflare *retains* logs (~3 days) and makes them queryable in the dashboard. This is the after-the-fact analysis layer (Task 1).
3. Logpush — long-term/SIEM export. Out of scope.

The `http.in` / `http.out` log lines (Task 3) guarantee *every* request — including ones rejected early (403 bad secret, 404) — appears with a correlation id, so a single message's whole journey can be stitched with `grep rid=<id>`.

## File Structure

- **Modify** `crates/worker/src/observability.rs` — add `Level`, `id_from_ray`/`request_id`, `level_for_status`, `trace_enabled`, `log`, `log_request_in`, `log_request_out`, `log_error`; keep existing `log_event` for back-compat. Add `#[cfg(test)]` unit tests for the pure helpers.
- **Modify** `crates/worker/src/lib.rs` — add `#[event(start)]` panic hook; wrap the fetch entry to log request-in, request-out (with status + latency), and router errors.
- **Modify** `crates/worker/src/middleware.rs` — log a structured `error` event for any `5xx` produced by `error_with_cors`.
- **Modify** `crates/worker/src/error.rs` — route `AppError::Internal` through `observability::log_error`.
- **Modify** `crates/worker/src/routes/webhook_telegram.rs` — derive the correlation id, emit a `tg.message` event always and a `tg.parse` trace gated by `DEBUG_TRACE`.
- **Modify** `crates/worker/Cargo.toml` — add `console_error_panic_hook`.
- **Modify** `wrangler.toml` — add `[observability]`.
- **Modify** `docs/debugging.md` — document the new events, correlation id, and Workers Logs.

**Convention reminder (CLAUDE.md):** feature-branch commits use gitmoji and bypass the hook:
`SKIP=commitizen git commit --no-verify -m "✨ <Subject>"`.

**Host test command (CLAUDE.md):**
```bash
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe test \
  --target x86_64-pc-windows-msvc -p grumps-worker
```

---

### Task 1: Panic hook + Workers Logs retention

**Files:**
- Modify: `crates/worker/Cargo.toml` (`[dependencies]`)
- Modify: `crates/worker/src/lib.rs:24` (add `#[event(start)]` above the existing `#[event(scheduled)]`)
- Modify: `wrangler.toml` (append `[observability]`)

- [ ] **Step 1: Add the panic-hook dependency**

In `crates/worker/Cargo.toml`, under `[dependencies]`, after the `hex = "0.4"` line, add (same version as the SPA, already in `Cargo.lock`):

```toml
console_error_panic_hook = "0.1"
```

- [ ] **Step 2: Install the panic hook at worker start**

In `crates/worker/src/lib.rs`, immediately above the existing `#[event(scheduled)]` (line 26), add:

```rust
/// Install a WASM panic hook so a panic surfaces a readable
/// `panicked at 'msg', file:line` line in `wrangler tail` / Workers Logs
/// instead of an opaque `RuntimeError: unreachable executed`.
#[event(start)]
fn start() {
    console_error_panic_hook::set_once();
}
```

- [ ] **Step 3: Enable Workers Logs**

In `wrangler.toml`, append at the end of the file:

```toml
# Retain logs in Cloudflare (queryable in the dashboard, ~3 days) instead of
# only the ephemeral `wrangler tail` stream. head_sampling_rate = 1.0 keeps
# 100% of requests — drop it if volume ever gets expensive.
[observability]
enabled = true
head_sampling_rate = 1.0
```

- [ ] **Step 4: Verify it compiles**

Run:
```bash
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe build \
  --target x86_64-pc-windows-msvc -p grumps-worker
```
Expected: builds with no errors.

- [ ] **Step 5: Commit**

```bash
git add crates/worker/Cargo.toml crates/worker/src/lib.rs wrangler.toml
SKIP=commitizen git commit --no-verify -m "✨ Add worker panic hook and enable Workers Logs"
```

---

### Task 2: Observability core helpers (TDD)

**Files:**
- Modify: `crates/worker/src/observability.rs`
- Test: same file, `#[cfg(test)] mod tests`

- [ ] **Step 1: Write the failing tests**

Append to `crates/worker/src/observability.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_from_ray_uses_ray_when_present() {
        assert_eq!(id_from_ray(Some("8abc123-DFW")), "8abc123-DFW");
    }

    #[test]
    fn id_from_ray_falls_back_when_missing_or_empty() {
        assert_eq!(id_from_ray(None), "local");
        assert_eq!(id_from_ray(Some("")), "local");
    }

    #[test]
    fn level_for_status_maps_ranges() {
        assert_eq!(level_for_status(200).as_str(), "info");
        assert_eq!(level_for_status(302).as_str(), "info");
        assert_eq!(level_for_status(404).as_str(), "warn");
        assert_eq!(level_for_status(403).as_str(), "warn");
        assert_eq!(level_for_status(500).as_str(), "error");
        assert_eq!(level_for_status(503).as_str(), "error");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run:
```bash
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe test \
  --target x86_64-pc-windows-msvc -p grumps-worker observability
```
Expected: FAIL — `cannot find function id_from_ray` / `level_for_status` / `Level`.

- [ ] **Step 3: Implement the helpers**

Replace the entire contents of `crates/worker/src/observability.rs` *above* the `#[cfg(test)]` block with:

```rust
use serde::Serialize;
use serde_json::json;

/// Log severity. Lowercased in output so `wrangler tail` / Workers Logs can be
/// grepped/filtered by `[info]` / `[warn]` / `[error]`.
#[derive(Clone, Copy)]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Info => "info",
            Level::Warn => "warn",
            Level::Error => "error",
        }
    }
}

/// Map an HTTP status to a log level: 5xx = error, 4xx = warn, else info.
pub fn level_for_status(status: u16) -> Level {
    if status >= 500 {
        Level::Error
    } else if status >= 400 {
        Level::Warn
    } else {
        Level::Info
    }
}

/// Derive a per-request correlation id. Prefers Cloudflare's `cf-ray` (unique
/// per request in prod). Falls back to "local" under `wrangler dev`, where
/// cf-ray is absent — webhook handlers add the platform message id to the log
/// fields so concurrent local replays stay distinguishable.
pub fn id_from_ray(ray: Option<&str>) -> String {
    match ray {
        Some(r) if !r.is_empty() => r.to_string(),
        _ => "local".to_string(),
    }
}

/// Extract the correlation id from a request's `cf-ray` header.
pub fn request_id(req: &worker::Request) -> String {
    id_from_ray(req.headers().get("cf-ray").ok().flatten().as_deref())
}

/// True when the `DEBUG_TRACE` env var is set to "1". Gates verbose per-branch
/// trace logs so production stays quiet; flip the var (Cloudflare dashboard or
/// `.dev.vars`) without redeploying logic.
pub fn trace_enabled(env: &worker::Env) -> bool {
    env.var("DEBUG_TRACE")
        .map(|v| v.to_string() == "1")
        .unwrap_or(false)
}

/// Core structured log line: `[level] event rid=<id> {json}`.
pub fn log(level: Level, rid: &str, event: &str, fields: &serde_json::Value) {
    worker::console_log!("[{}] {} rid={} {}", level.as_str(), event, rid, fields);
}

/// One line per inbound request, before routing.
pub fn log_request_in(rid: &str, method: &str, path: &str) {
    log(Level::Info, rid, "http.in", &json!({ "method": method, "path": path }));
}

/// One line per request after routing, with status + latency. Level follows
/// the status code so `[error]` flags 5xx.
pub fn log_request_out(rid: &str, status: u16, ms: u64) {
    log(level_for_status(status), rid, "http.out", &json!({ "status": status, "ms": ms }));
}

/// Structured error event with where-it-happened context.
pub fn log_error(rid: &str, location: &str, detail: &str) {
    log(Level::Error, rid, "error", &json!({ "where": location, "detail": detail }));
}

/// Log a structured event for easy grep in `wrangler tail`. Serialized as a
/// single JSON line prefixed with `[event]`. Retained for existing call sites.
pub fn log_event<T: Serialize>(event: &str, fields: &T) {
    if let Ok(json) = serde_json::to_string(fields) {
        worker::console_log!("[event] {} {}", event, json);
    } else {
        worker::console_log!("[event] {}", event);
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run:
```bash
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe test \
  --target x86_64-pc-windows-msvc -p grumps-worker observability
```
Expected: PASS — `id_from_ray_uses_ray_when_present`, `id_from_ray_falls_back_when_missing_or_empty`, `level_for_status_maps_ranges`.

- [ ] **Step 5: Commit**

```bash
git add crates/worker/src/observability.rs
SKIP=commitizen git commit --no-verify -m "✨ Add leveled structured logging helpers and correlation id"
```

---

### Task 3: Request-in/out + error logging at the fetch entry

**Files:**
- Modify: `crates/worker/src/lib.rs:34-120` (the `#[event(fetch)] pub async fn main`)

- [ ] **Step 1: Wrap the fetch entry point**

In `crates/worker/src/lib.rs`, change the start of `main` from:

```rust
#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    // Handle CORS preflight for all paths
    if req.method() == Method::Options {
        return middleware::preflight(&req);
    }

    Router::new()
```

to:

```rust
#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    // CORS preflight: answered before logging to keep OPTIONS noise out.
    if req.method() == Method::Options {
        return middleware::preflight(&req);
    }

    // Correlation id (cf-ray) + request-in line, so every request — including
    // ones rejected early (403/404) — is visible and stitchable by `rid=`.
    let rid = observability::request_id(&req);
    let method = req.method().to_string();
    let path = req.path();
    let started = Date::now().as_millis();
    observability::log_request_in(&rid, &method, &path);

    let result = Router::new()
```

- [ ] **Step 2: Log request-out / error after the router runs**

In the same function, find the end of the router chain — the existing terminal `.run(req, env).await` call. Change it from:

```rust
        .run(req, env)
        .await
}
```

to:

```rust
        .run(req, env)
        .await;

    let ms = Date::now().as_millis().saturating_sub(started);
    match &result {
        Ok(resp) => observability::log_request_out(&rid, resp.status_code(), ms),
        Err(e) => observability::log_error(&rid, "fetch", &e.to_string()),
    }
    result
}
```

> Note: `Date` and `Response::status_code()` come from the existing `use worker::*;` at the top of `lib.rs` — no new import. The router chain currently *returns* `.run(...).await` directly; assigning it to `result` is the only structural change.

- [ ] **Step 3: Verify it compiles**

Run:
```bash
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe build \
  --target x86_64-pc-windows-msvc -p grumps-worker
```
Expected: builds with no errors.

- [ ] **Step 4: Behavioural verification (live)**

In terminal A:
```bash
npx wrangler dev
```
In terminal B:
```bash
./replay-webhook.sh health
./replay-webhook.sh tg "@grumps list"
```
Expected in terminal A: for each call, an `[info] http.in rid=... {"method":...,"path":...}` line followed by an `[info] http.out rid=... {"status":200,"ms":...}` line with the **same `rid`**. (Under `wrangler dev`, `rid=local`; that is expected — see Task 2 fallback.)

- [ ] **Step 5: Commit**

```bash
git add crates/worker/src/lib.rs
SKIP=commitizen git commit --no-verify -m "✨ Log every request in/out with correlation id and latency"
```

---

### Task 4: Structured logging for 5xx and AppError

**Files:**
- Modify: `crates/worker/src/middleware.rs:211-217` (`error_with_cors`)
- Modify: `crates/worker/src/error.rs:18` (`AppError::Internal` arm)

- [ ] **Step 1: Log 5xx from `error_with_cors`**

In `crates/worker/src/middleware.rs`, change `error_with_cors` from:

```rust
pub fn error_with_cors(req: &Request, status: u16, code: &str, detail: &str) -> Result<Response> {
    let body = serde_json::json!({ "error": code, "detail": detail });
    let mut resp = Response::from_json(&body)?.with_status(status);
```

to:

```rust
pub fn error_with_cors(req: &Request, status: u16, code: &str, detail: &str) -> Result<Response> {
    // Server-side faults get a structured error line tagged with the request's
    // correlation id; client errors (4xx) stay as the http.out warn line only.
    if status >= 500 {
        let rid = crate::observability::request_id(req);
        crate::observability::log_error(&rid, code, detail);
    }
    let body = serde_json::json!({ "error": code, "detail": detail });
    let mut resp = Response::from_json(&body)?.with_status(status);
```

- [ ] **Step 2: Route `AppError::Internal` through `log_error`**

In `crates/worker/src/error.rs`, change the `Internal` arm from:

```rust
            Self::Internal(m) => {
                worker::console_log!("Error: {}", m);
                Response::error("Internal error", 500)
            }
```

to:

```rust
            Self::Internal(m) => {
                // No request handle here, so the id is "unknown"; the http.out
                // line for this request carries the real rid alongside.
                crate::observability::log_error("unknown", "AppError::Internal", &m);
                Response::error("Internal error", 500)
            }
```

- [ ] **Step 3: Verify it compiles**

Run:
```bash
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe build \
  --target x86_64-pc-windows-msvc -p grumps-worker
```
Expected: builds with no errors.

- [ ] **Step 4: Behavioural verification (live)**

With `wrangler dev` running, send a Telegram webhook with a **wrong secret** (forces the 403 path — not 5xx, so no `[error]` line, but confirms `http.out` warns):
```bash
TG_WEBHOOK_SECRET=wrong ./replay-webhook.sh tg "hello"
```
Expected in `wrangler dev`: `[info] http.in ...` then `[warn] http.out rid=... {"status":403,...}`. (A genuine 5xx would additionally produce an `[error] error rid=... {"where":...}` line.)

- [ ] **Step 5: Commit**

```bash
git add crates/worker/src/middleware.rs crates/worker/src/error.rs
SKIP=commitizen git commit --no-verify -m "✨ Emit structured error events for 5xx and AppError"
```

---

### Task 5: Correlation id + branch trace in the Telegram path

**Files:**
- Modify: `crates/worker/src/routes/webhook_telegram.rs` (the text-message branch around line 305, where `parser::parse` and `handler::handle_message` are called)

- [ ] **Step 1: Confirm `ParseResult` derives `Debug`**

Run:
```bash
grep -n "enum ParseResult" crates/nlu/src/*.rs
```
Then open the file and confirm the `enum ParseResult` has `#[derive(Debug, ...)]`. If `Debug` is absent, add it to the derive list before continuing (it is needed for the `{:?}` format below).

- [ ] **Step 2: Add the correlation id and `tg.message` event before parsing**

In `crates/worker/src/routes/webhook_telegram.rs`, immediately *before* the existing `let parse_result = parser::parse(` call (line ~305), insert:

```rust
    let rid = crate::observability::request_id(&req);
    crate::observability::log(
        crate::observability::Level::Info,
        &rid,
        "tg.message",
        &serde_json::json!({
            "msg_id": inbound.message_id,
            "is_mention": inbound.is_mention_to_bot,
            "is_dm": inbound.is_direct_message,
        }),
    );
```

- [ ] **Step 3: Add the gated `tg.trace` event (full content + replayable body) after parsing**

Immediately *after* the existing `let parse_result = parser::parse(...);` block (after line ~311) and *before* the `let llm = ...` line, insert. This logs the **full message content** (sender + text), the chosen parse variant, and the **raw update body** — the last is exactly what `replay-webhook.sh` sends, so a logged failure can be re-fed verbatim. Gated by `DEBUG_TRACE` so it is off in normal operation. (`body` is the `Vec<u8>` read at the top of `handle_incoming`; `text` and `inbound` are in scope here. Use references in the `json!` so nothing is moved out of `inbound`, which is read again below.)

```rust
    if crate::observability::trace_enabled(&ctx.env) {
        crate::observability::log(
            crate::observability::Level::Info,
            &rid,
            "tg.trace",
            &serde_json::json!({
                "sender": &inbound.sender_name,
                "text": text,
                "variant": format!("{parse_result:?}"),
                "raw": String::from_utf8_lossy(&body),
            }),
        );
    }
```

> Privacy note: `tg.trace` puts real message content into Cloudflare Workers Logs (retained ~3 days, dashboard-visible). That is acceptable here — it is gated behind `DEBUG_TRACE`, the app is pre-production with no real user data, and nothing reaches the committed repo. Anonymisation/redaction can be layered into this single helper later if the app goes live; keeping all content logging in this one gated event is what makes that a one-place change.

- [ ] **Step 4: Verify it compiles**

Run:
```bash
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe build \
  --target x86_64-pc-windows-msvc -p grumps-worker
```
Expected: builds with no errors.

- [ ] **Step 5: Behavioural verification (live)**

Add `DEBUG_TRACE="1"` to `.dev.vars`, then with `wrangler dev` running:
```bash
./replay-webhook.sh tg "buy bread tomorrow 9am"
```
Expected in `wrangler dev`: an `[info] tg.message rid=... {"msg_id":...}` line and an `[info] tg.trace rid=... {"sender":"Alice","text":"buy bread tomorrow 9am","variant":"AddSingleTodo(...)","raw":"{...full update...}"}` line, both sharing the same `rid` as the surrounding `http.in`/`http.out`. Remove `DEBUG_TRACE` from `.dev.vars` and re-run: the `tg.trace` line disappears, `tg.message` stays.

- [ ] **Step 6: Commit**

```bash
git add crates/worker/src/routes/webhook_telegram.rs crates/nlu
SKIP=commitizen git commit --no-verify -m "✨ Trace Telegram message content and replayable body with correlation id"
```

---

### Task 6: Document the new observability surface

**Files:**
- Modify: `docs/debugging.md`

- [ ] **Step 1: Append a new section**

Add to the end of `docs/debugging.md` (keep prose unwrapped, one flowing line per paragraph — repo convention):

```markdown
## Strengthening diagnosis when something goes wrong

Every request now emits a correlation id derived from Cloudflare's `cf-ray` header (falls back to `local` under `wrangler dev`). Grep one request's whole journey with `wrangler tail | grep rid=<id>`.

Structured log lines, all greppable by `[level]` and `rid=`:

| Event | When | Fields |
|---|---|---|
| `http.in` | every request, before routing | `method`, `path` |
| `http.out` | every request, after routing | `status`, `ms` (latency); level is `error` for 5xx, `warn` for 4xx |
| `error` | any 5xx or `AppError::Internal` | `where`, `detail` |
| `tg.message` | every inbound Telegram text | `msg_id`, `is_mention`, `is_dm` (metadata only, no content) |
| `tg.trace` | Telegram text, only when `DEBUG_TRACE=1` | `sender`, `text` (full content), `variant` (chosen `ParseResult`), `raw` (the full update body — replayable verbatim via `replay-webhook.sh`) |

Flip verbose content tracing on without redeploying: set `DEBUG_TRACE=1` (Cloudflare dashboard env var in prod, or `.dev.vars` locally). `tg.message` is always on and content-free; `tg.trace` carries the full message content and is gated.

Privacy: `tg.trace` writes real message content to Workers Logs (retained ~3 days, dashboard-visible) — fine pre-production, and easy to redact later since all content logging lives in that one gated event. Nothing is ever written to the committed repo.

### Retained logs (analysis after the fact)

`wrangler tail` only shows what streams while you are connected. Workers Logs is enabled (`[observability]` in `wrangler.toml`), so Cloudflare retains logs (~3 days) and makes them queryable in the dashboard: Workers → `grumps-api` → Logs. Filter by `rid=`, status, or `[error]` to find a specific failure after it happened.

### Panics

The worker installs `console_error_panic_hook` at start, so a panic surfaces a readable `panicked at 'msg', file:line` line instead of `RuntimeError: unreachable executed`. (The SPA already had its own hook in `crates/spa/src/main.rs` — that one covers the browser, this one covers the worker.)
```

- [ ] **Step 2: Commit**

```bash
git add docs/debugging.md
SKIP=commitizen git commit --no-verify -m "📝 Document worker correlation id, structured logs, Workers Logs"
```

---

## Self-Review notes

- **Coverage:** "see all received requests" → Task 1 (Workers Logs retention) + Task 3 (`http.in`/`http.out` per request). "Identify errors" → Task 3 (router errors), Task 4 (5xx + AppError), Task 1 (panic hook), Task 2 (correlation id to stitch lines). Telegram branch visibility → Task 5.
- **Type consistency:** `Level`, `level_for_status`, `id_from_ray`, `request_id`, `trace_enabled`, `log`, `log_request_in`, `log_request_out`, `log_error` are all defined in Task 2 and used with matching signatures in Tasks 3–5. `log_event` is preserved so the ~50 existing call sites keep compiling.
- **Known soft spots flagged inline:** `AppError::Internal` has no request handle (id `"unknown"`) and is currently called nowhere — improved for consistency only. `ParseResult: Debug` is verified in Task 5 Step 1 rather than assumed.
- **Content logging:** full message content + replayable raw body live in the single `tg.trace` event, gated by `DEBUG_TRACE` — the one place to add anonymisation later. Same pattern can be extended to the WhatsApp/Discord handlers if needed.
```
