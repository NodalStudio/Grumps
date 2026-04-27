# E2E Test Findings — 2026-04-26

Driven via Playwright on local `wrangler dev` + `trunk serve` with seeded
test workspaces (Roommates 5-member group + 6 todos + 2 notes + 3 pinned
memories, Personal DM, Old Group archived).

## Suite status

129 passing, 1 skipped (DO alarm — deployed-only). 21 spec files.

| File | Tests | Notes |
|---|---|---|
| `auth.spec.ts` | 4 | login page, /auth/me 401, dev_bypass, logout |
| `bad-input.spec.ts` | 6 | malformed JSON returns 400 (not 500) on every mutating route |
| `billing.spec.ts` | 1 | quota cap blocks 26th todo, locale switched to fr |
| `dashboard.spec.ts` | 5 | seeded workspaces, archived chip, navigation |
| `events.spec.ts` | 3 | list, CRUD, calendar feed integration |
| `export.spec.ts` | 4 | todos.json, todos.csv, notes.json, isolation |
| `history.spec.ts` | 3 | activity log array, limit cap, todo logging |
| `i18n.spec.ts` | 2 | day initials in EN and FR for the "This week" widget |
| `ical.spec.ts` | 2 | create + delete iCal token, CSRF guard |
| `ical-feed.spec.ts` | 3 | iCal feed token gate + valid feed + revocation |
| `isolation.spec.ts` | 4 | cross-workspace 403/404, multi-workspace access |
| `members.spec.ts` | 4 | list 5 members, role, 403 for non-member, DM has 1 |
| `memory.spec.ts` | 5 | list seeded, kind filter, CRUD, CSRF, "What I know" UI |
| `notes.spec.ts` | 4 | list, CRUD round-trip, CSRF guard, UI listing |
| `scheduled.spec.ts` | 2 | list, create + delete reminder |
| `sessions.spec.ts` | 2 | list current session, revoke-all-others |
| `settings.spec.ts` | 6 | locale + name + display_name PATCHes, validation |
| `todos-api.spec.ts` | 8 | create + list + delete + patch (status/title/assignee), CSRF, 403 |
| `todos.spec.ts` | 6 | UI: filters, checkbox toggle (idempotent), Add Todo focus, Enter creates |
| `webhook-telegram.spec.ts` | 3 | secret token enforcement (mandatory + value), unknown chat is no-op |
| `webhook-todo.spec.ts` | 5 | TODO/NOTE end-to-end, EN + FR memory triggers, KV dedup |
| `workspace-pages.spec.ts` | 13 | smoke for all workspace pages, calendar header, agenda view |
| `workspace.spec.ts` | 5 | overview header, stats, sidebar footer, switcher |

## Bugs found and fixed

- **Archived workspace card had no visual marker** — fixed in
  `crates/spa/src/pages/dashboard.rs`: dashed border, opacity-60, chip.

- **Workspace overview header showed slug instead of name** — fixed by
  having `get_workspace_info` return `WorkspaceOverview { slug, name,
  plan, stats }` and deriving the title reactively from the
  `LocalResource` (the previous `provide_context(SessionContext)` read
  was non-reactive).

- **Workspace stats all showed 0 despite seeded data** — API returns
  `{slug,name,plan,stats:{...}}` but SPA expected flat fields. Fixed
  with the `WorkspaceOverview` struct + `#[serde(default)]` on every
  `StatusCounts` field.

- **`/auth/me` returned `auth.invalid_token` when no credentials were
  presented at all** — fixed in `crates/worker/src/middleware.rs`:
  detect "no cookie + no Authorization header" upfront and return
  `auth.unauthenticated`.

- **User footer hardcoded "MEMBER" + "You"** — fixed in
  `crates/spa/src/components/sidebar.rs` to read from session.

- **POST /events, /memory, /scheduled returned 500** — `created_by`
  is FK → `members(id)` but the worker was inserting `claims.sub`
  (a global user-id, not workspace member-id). Fixed by adding
  `WorkspaceDb::find_member_by_platform_id` and resolving via
  `claims.tg_user_id`, falling back to NULL when not a member.

- **"This week" calendar showed French day initials in every locale** —
  `crates/spa/src/pages/overview.rs` had a hardcoded
  `["L","M","M","J","V","S","D"]`. Fixed by adding
  `overview.day.short.{mon..sun}` keys for all 14 locales and reading
  them via `tr()`. Verified with EN + FR in `i18n.spec.ts`.

- **Agent fast-path commands were French-only and replies were
  hardcoded French strings** — `try_fast_commands` matched only
  `tais-toi` / `souviens-toi …` / `reviens` and replied with literal
  `🤫 Silence pour 24h` / `💾 Noté !` etc. Fixed by:
  – Resolving the workspace's locale at the start of the fast-path.
  – Adding `agent.silence.confirm`, `agent.silence.unset`,
    `agent.memory.saved`, `agent.memory.error`, `agent.error.technical`
    keys to all 14 locales.
  – Accepting equivalents in many languages (EN: "remember that",
    "remember to", "note that", "quiet", "come back"; same family
    extensions for the other locales).
  – Replacing every literal user-facing string with `t(locale, key)`.

- **A clutch of mutating routes accepted invalid input** without
  validation, persisting questionable rows. Fixed:
  - `POST /todos` now rejects `priority` outside 1..3 (also corrected
    the default from `3 (Low)` to `2 (Normal)`).
  - `POST /todos` and `PATCH /todos/<id>` now cap title at 500 chars
    (a 5000-char title was being persisted before).
  - `PATCH /todos/<id>` now rejects unknown `status` (only
    `open|in_progress|done`) and rejects empty title.
  - `POST /memory` now rejects whitespace-only `value`.
  - `PATCH /api/me` now rejects empty or >80-char `display_name`.

- **No way to test (or recover from) a stuck scheduler** — DO alarms
  in `wrangler dev` don't fire reliably (no `alarm()` invocation in
  90s+ past `trigger_at`), and there was no manual escape hatch in
  prod either. Added `POST /api/admin/w/:slug/scheduled/:id/fire`
  (super-admin only) that calls `scheduler_executor::execute_action`
  directly. Doubles as a test hook and an ops kill-switch.

- **`POST /api/w/<slug>/scheduled` accepted past `trigger_at`** — a
  non-recurring past trigger never fires, and a recurring one could
  fire on every tick while the runtime catches up. Fixed with a 60s
  grace future-only check (and a non-empty-title check).

- **`POST /api/w/<slug>/events` accepted `ends_at` before `starts_at`
  and an empty title** — calendar would silently persist negative-
  duration events. Fixed by adding range + non-empty checks in
  `events::create` and `events::update`; returns 400 `bad_request`.

- **A handful of small SPA strings were still hardcoded English** —
  fixed `dashboard.rs` "Archived" chip → `dashboard.workspace.archived`
  (i18n in all 14 locales), `global_settings.rs` "Last active …" →
  `settings.last_active`, "Save" button → `common.save`, and
  `memory_card.rs` "AUTO" badge → `memory.badge.auto`. The dashboard
  test now forces `?lang=en` to keep the assertion deterministic.

- **The remaining handler.rs bot replies were hardcoded English** —
  `handle_card_reply` (Done./Deleted./Snoozed/Updated/Reassigned/
  Priority/#Tag/Status), `handle_complete_todos`/`handle_complete_single`
  (✅ #N "{title}" — done., recurrence note, fuzzy-match prompt,
  no-match), `SetReminder` (⏰ Reminder set …), and `Summarize`
  (Summarize is coming soon!) all lived as literal English in
  `handler.rs`. Fixed by adding `agent.card.{done,next_occurrence,
  deleted,snoozed,updated,reassigned,priority,tag_added,status}`,
  `agent.todo.{completed,fuzzy_match_header,fuzzy_match_footer,
  no_match}`, `agent.reminder.{set,target_for,default_title}`,
  `agent.summarize.coming_soon` keys for all 14 locales and threading
  `locale` through every handler. The codebase no longer has any
  literal user-facing English string in `handler.rs`.

- **Note editor never persisted edits** — `pages/note_editor.rs` had a
  textarea with `on:input` updating a local signal, but no Save
  button and no API call. Reloading lost everything. Fixed by adding
  a Save button bound to `api.update_note(...)`, a Title input, and a
  Saving/Saved/Save-failed indicator. All UI strings localized
  (`page.note_editor.{untitled,created,edit,preview,save,saving,
  saved,save_error,not_found}` + `common.title.placeholder`).

- **Bot reply for `delete #N`, `quoted todo`, `quoted note`,
  `note saved`, `which todo to delete` were hardcoded English** —
  fixed by adding `agent.todo.{deleted,not_found,which_to_delete}`,
  `agent.note.saved`, `agent.quoted.{no_todo,no_note}` keys for all
  14 locales and threading `locale` through `handle_delete`,
  `handle_add_note`, and the LLM-result branch.

- **Mutating routes returned 500 on malformed JSON** — every
  `req.json()` deserialization error surfaced as `Error::RustError`
  (→ 500). Fixed in `routes/{notes,todos,memory,events,scheduled}.rs`
  to return a proper 400 with `{"error":"bad_request"}`.

- **Bot replies for list/notes/status were locked to English** —
  `formatter::status_summary`, `todo_list`, `note_list`,
  `todos_added_summary`, `task_card` were all called with literal
  `"en"`. Fixed by threading the workspace locale into the relevant
  handlers and passing `locale.code()` instead. Also localized
  `agent.files.web_only` and `agent.notes.search_empty` for all 14
  locales.

- **Billing quota messages were hardcoded English** —
  `check_todo_quota`, `check_note_quota`, `check_llm_quota` returned
  `Err(String)` baked in English ("Todo limit reached…"). Fixed by
  refactoring to return a typed `QuotaError` enum with
  `render(locale)`, threading `&workspace.locale` through
  `handle_message`, and adding `billing.quota.{todos,notes,llm}` keys
  to all 14 locales. Workspace locale now flows from the index DB
  through every handler invocation.

## Remaining items

- Files page (placeholder; backend endpoint missing — known)
- Note editor UI (only API tested)
- Scheduled actions UI page (only API tested)
- Calendar drag-and-drop (TODO comment in source)
- Admin observability surfaces (super-admin gate verified, content
  not exercised since seed user isn't a super admin)
- Multi-user concurrent edits
- Agent LLM round-trip (`@HeyGrumpsBot <free-form>` → Sonnet/Gemini reply)
  — fast-path covered, full LLM route still untested

## How to run

```bash
# Worker
PATH="/c/Users/mayer/.cargo/bin:$PATH" \
  npx wrangler dev --port 8787 --var ENVIRONMENT:development

# SPA
cd crates/spa && trunk serve --release --port 8080

# Tests
cd tests && npm test
```

Tests share the local D1 sandbox; create-and-delete patterns keep runs
idempotent. Reset by re-running the seed scripts under `scripts/`.
