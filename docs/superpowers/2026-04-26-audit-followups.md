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
Six `formatter::*` calls (lines 103, 125, 225, 230, 238, 244) pass the
literal `"en"`, plus two raw English strings (lines 528, 532) violate
the i18n hard rule. Thread `WorkspaceMetaRow.locale` through the
`handle_message` signature and replace the literal calls. Add i18n keys
for the two raw strings (`bot.delete.needs_seq`,
`bot.summarize.coming_soon`).

### #8 — `messaging_dispatch.rs` bypasses the messaging crate
`crates/worker/src/messaging_dispatch.rs:18-33` builds Telegram HTTP
requests inline rather than calling `TelegramAdapter::send` from
`grumps-messaging`. Comments still reference internal "Plan A" /
"Plan B" identifiers (banned per CLAUDE.md commit-message rule). Route
through the adapter; replace the comments with neutral phrasing.

### #9 — `crates/worker/src/db.rs` is 1660 LOC mixing two DBs
The file holds both Index-DB queries (`D1Database` binding) and
Workspace-DB queries (`D1RestClient`). Split into `db/index.rs` and
`db/workspace.rs`, re-export from `db/mod.rs`. The split makes the
type-level boundary between the two databases explicit so a future
contributor can't accidentally route a workspace query into the index.

### #10 — Dual auth state in the SPA (`AuthState` and `SessionContext`)
`crates/spa/src/auth/mod.rs` carries a full `AuthState` struct alongside
the newer `SessionContext`. `app.rs:20` calls `provide_auth()`
unconditionally as a "no-op placeholder" outside demo mode. The plan was
to delete `AuthState` after migration; finish or deprecate explicitly.

### #11 — Demo-mode guards scattered through `crates/spa/src/api.rs`
Currently 15+ `if crate::demo::is_demo() { return ...; }` checks inline.
Refactor to `trait Api` with `DemoApi` and `LiveApi` implementations,
chosen at startup and provided as `Arc<dyn Api>`. Removes every inline
guard and makes "forgot a demo branch" impossible.

### #12 — Plan quota limits duplicated 3+ times
`crates/agent/src/loop_.rs` lines 24, 143, 253 (agent calls) and 262-264
(web search) each match `plan` against the same numeric ladder. Extract
to a `fn limits_for_plan(plan: &str) -> QuotaLimits` helper in
`crates/core/src/billing.rs` and call from all sites.

### #13 — SPA i18n violations
Hardcoded English strings in:
- `components/calendar/agenda.rs:18` — `"Nothing scheduled."`
- `pages/global_observability.rs` lines 65, 119, 183, 300, 301
- `pages/note_editor.rs:60` — `"Note not found."`
- `app.rs:25` — `"404 — Not found."`

Add keys to `crates/i18n/locales/en.json` and use `tr(...)`.

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
