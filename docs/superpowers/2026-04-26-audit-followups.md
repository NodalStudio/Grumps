# 2026-04-26 — Audit follow-ups

This file tracks the items surfaced by the 2026-04-26 audit that were
deferred from the immediate fix PR. Cross-reference: the
audit produced four critical findings (#1, #3, #4, #5 in the original
list) which are addressed in the same PR as this document. Everything
below is queued for later.

---

## Deferred (intentional, no action expected)

### #2 — Live Telegram bot token in `.dev.vars`
The file is gitignored and not tracked. The author keeps the live token
locally for manual webhook tests, accepting the risk. If the laptop is
ever compromised, rotate `TG_BOT_TOKEN` and `TG_WEBHOOK_SECRET` via
`wrangler secret put` and replace the local copy.

### #6 — WhatsApp OTP rate limiting
Skipped because WhatsApp is not currently active. When the WA path is
re-enabled, add `check_rate_limit(&ctx.env, &req, "otp_send", 5).await`
at the top of `handle_send_otp` in `crates/worker/src/routes/auth.rs`.

---

## Code-quality / architecture (medium priority)

### #7 — Locale hardcoded to `"en"` in `crates/worker/src/handler.rs`
✅ Done in 9cd103a (validate-mutating-routes pass). `handle_message`
takes a typed `Locale` and threads it through every helper; the two
raw English strings now route through `agent.todo.which_to_delete` /
`agent.summarize.coming_soon`.

### #8 — `messaging_dispatch.rs` bypasses the messaging crate
✅ Done in 2026-04-27 audit-medium pass. Routes through
`TelegramAdapter::build_send_request`; "Plan A/B" comments removed.

### #9 — `crates/worker/src/db.rs` is 1660 LOC mixing two DBs
✅ Done in 2026-04-27 audit-medium pass. Split into `db/index.rs`
(300 LOC) and `db/workspace.rs` (1387 LOC) with `db/mod.rs`
re-exporting both.

### #10 — Dual auth state in the SPA (`AuthState` and `SessionContext`)
✅ Done in 2026-04-27 audit-medium pass. `AuthState`,
`provide_auth`, `use_auth` deleted. `ApiClient` is now a unit
struct constructed at each callsite. `SessionContext` remains
as the single auth context.

### #11 — Demo-mode guards scattered through `crates/spa/src/api.rs`
**DEFERRED**. The trait-based refactor (`trait Api` with
`async-trait`, `DemoApi` + `LiveApi` impls, `Rc<dyn Api>` factory)
touches 30+ method signatures and 13 callsites. Worth doing on its
own PR with focused review; out of scope for this batch.

### #12 — Plan quota limits duplicated 3+ times
✅ Done in 2026-04-27 audit-medium pass. The `Plan` enum moved to
`crates/core/src/billing.rs` with `agent_call_quota()` and
`web_search_quota()` methods. Worker re-exports from core. Agent
crate uses `Plan` directly; old hardcoded ladders deleted. Also
caught a 4th hardcoded French quota string in `tools/web.rs` and
moved it to `agent.web_search.quota_exceeded`.

### #13 — SPA i18n violations
✅ Mostly done in 9cd103a (which added `calendar.agenda.empty`,
`common.404`, `page.note_editor.not_found`); finished in
2026-04-27 audit-medium pass with `observability.no_data`,
`observability.no_workspaces`,
`observability.no_quality_signals`,
`observability.access_denied`,
`observability.super_admin_only` (5 keys × 14 locales) and the
matching `tr()` calls in `pages/global_observability.rs`.

### #14 — Observability is one `console_log!` wrapper
`crates/worker/src/observability.rs` is 11 lines, called from only 9
sites. There is no per-request `request_id` and no span hierarchy, so
`wrangler tail` shows an unordered stream that can't be correlated
through agent loop → tool calls → DB writes. Generate a
`Uuid::new_v4()` at the top of each webhook handler, thread through
`ToolContext`, and emit it in every `log_event` / `console_log!` call.

### #15 — No `[workspace.lints]` baseline
Add a `[workspace.lints]` table in the root `Cargo.toml`:
```toml
[workspace.lints.clippy]
unwrap_used = "warn"
pedantic = "warn"
```
Then `lints.workspace = true` in each crate's `Cargo.toml`. Pairs with
flipping the new `ci.yml` clippy steps to `-- -D warnings` (see #18
below).

### #16 — `crates/spa/Cargo.toml` doesn't use `workspace.dependencies`
Lines 14-15 declare `serde` and `serde_json` inline while every other
crate uses `.workspace = true`. Change to:
```toml
serde.workspace = true
serde_json.workspace = true
```

### #17 — Dead code (22 warnings remaining after `cargo fix`)
`cargo check` flags the following as never used. Most are leftovers
from earlier auth iterations — confirm before deleting:
- `error::AppError` enum + `into_response` method (whole error type
  appears unused; routes return `worker::Result<Response>` directly)
- `middleware::check_workspace_access`, `middleware::is_workspace_admin`
- `middleware::AuthError::Unauthenticated` variant
- `db::UserIdentity` struct, `db::list_user_identities`,
  `db::get_member_role`, `db::cancel_reminder`, `db::count_memory`
- `db::ScheduledRow.message_id` field (never read)
- `db::AgentSessionRow.{seq_num, status}` fields (never read)
- `d1_rest::split_sql_statements`, `d1_rest::starts_with_word_ci`,
  `d1_rest::QueryResult.last_row_id`
- `billing::Plan::{as_str, max_storage_bytes, max_groups}` (whole impl)
- `routes::auth::OtpRequest.workspace_slug` field
- `scheduler_executor::WorkerConditionContext.db` field
- SPA: `api::{MemberItem, EventItem, OtpResponse, VerifyResponse}`,
  `auth::AuthState` whole, several `demo::` helpers, `i18n::tr_p`

### #18 — Tighten CI to deny warnings
Once #17 is cleaned, change the two clippy steps in
`.github/workflows/ci.yml` from `cargo clippy ...` to
`cargo clippy ... -- -D warnings` so future warnings break the build.

---

## Cleanup punch list (low priority, zero risk)

### #19 — Untracked stray Rust archive
`libnul.rlib` at the repo root (4.4 KB, currently *tracked*) — leftover
from a Windows build that misinterpreted `nul` as an output name. Zero
references in the repo. Run `git rm --cached libnul.rlib`.

### #20 — Root-level mockup PNGs
80 `.png` files at the repo root (~30 MB, gitignored, untracked).
`rm *.png` in the working tree to free space. None are referenced
outside other PNGs / markdown commentary.

### #21 — 13 byte-identical `workspace.{lang}.html` at the root
All 13 are MD5-equal to `workspace.html`. `landing/build.mjs:248-260`
already falls back to English when the file is missing. Run:
```
git rm workspace.{ar,de,es,fr,hi,id,it,ja,ko,pt-BR,ru,tr,zh-CN}.html
```
Drops ~1.2 MB and 13 entries from `git ls-files`.

### #22 — `dist/workspace.html`
Leftover from a build before `dist/` was added to `.gitignore`. Not
tracked. Safe to delete from the working tree.

### #23 — `test-webhook.sh`
Manual WhatsApp curl harness, tracked at the root. Currently
unreferenced; the active flow is Telegram. Move to `scripts/` or
delete.

### #24 — Stale superpowers plans (referencing "Plan A/B/C")
Five files in `docs/superpowers/plans/` from 2026-04-13 and 2026-04-19
predate the current architecture and use the now-banned "Plan A/B/C"
naming. Either archive to `docs/superpowers/archive/` or delete:
- `2026-04-13-phase1-chat-mvp.md`
- `2026-04-13-phase2-web-workspace.md`
- `2026-04-19-grumps-foundation-plan-A.md`
- `2026-04-19-grumps-agent-loop-plan-B.md`
- `2026-04-19-grumps-agent-calendar-plan-C.md`

---

## Privacy note

This is a public repo. The 2026-04-26 cleanup pass already replaced:
- the production Telegram user id (was `6108569905`) → `1234567890`
  in `crates/worker/src/auth_telegram.rs` test fixture
- real first/last name + Telegram username → `"Test"`, `"User"`,
  `"testuser"` in the same fixture
- `"Benoît only"` in CLAUDE.md → `"the project operator"`

The Dupont / `dupont@example.fr` / `+33 6 12 34 56 78` strings in
locale files and demo seed are intentional placeholder data (French
equivalent of "John Smith") and are safe to keep. Going forward, see
the **What MUST NOT land in committed files** section of CLAUDE.md
before adding new fixtures, examples, or doc snippets.
