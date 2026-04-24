# Unified multi-platform auth — design

Date: 2026-04-24
Status: Approved, ready for planning

## Overview

Grumps ships as a chat bot across Telegram, WhatsApp, and Discord, plus a web SPA at `grumps.app` that lets users see todos, notes, memory, calendar, and scheduled actions per workspace. Today the web SPA has a WhatsApp-only OTP login flow and no route guards — unauthenticated visitors can load `/w/:slug` and see a bare shell while all API calls fail with 500s (no CORS headers, bug pre-existing). Users who joined Grumps through Telegram have no web login path at all.

This design replaces the ad-hoc auth with a unified session model: first-class multi-platform identities (one Grumps user can link Telegram, WhatsApp, and Discord accounts), HttpOnly cookie sessions with CSRF protection, route-level auth gating in the SPA, a revocable sessions registry, and a `/login` page that presents three platforms side-by-side. In v1, only **Telegram Login Widget** is functionally wired. WhatsApp and Discord buttons render as "Bientôt" placeholders so the multi-platform promise is visible, but their mechanisms (OTP for WA, OAuth2 for Discord) are deferred.

The feature also fixes a gap: today, when a user is added to a Telegram group with Grumps, no row is created in `users` or `user_workspaces` — only the per-workspace `members` row. That means even the admin who added the bot cannot log into the SPA. Onboarding now writes the full identity graph.

## Goals

1. Telegram Login Widget integration — one click, HMAC-signed payload, no OTP
2. HttpOnly cookie session + CSRF double-submit (migration from localStorage JWT)
3. Schema supports multi-platform identity per user from day one (`user_identities` join table)
4. `users.phone` is no longer a required column — existing WA users are ported into `user_identities`
5. When a Telegram user is added to a group or promoted to admin, `users` + `user_identities` + `user_workspaces` are all populated
6. `chat.type == "private"` messages to the bot auto-provision a personal (DM) workspace with the user as sole admin
7. `workspaces_meta.name` is populated from platform data at provisioning time and editable from `/w/:slug/settings`
8. Workspace switcher dropdown in the SPA sidebar, sourced from the authenticated session's workspace list
9. New global `/settings` page: display name, default locale, linked identities, active sessions
10. `<AuthGate>` wrapper on protected routes — unauthenticated visitors are redirected to `/login?redirect=<path>`
11. Landing page hero CTA is dynamic — "Log in" for anonymous, "My workspaces" for authenticated
12. Empty dashboard for users with zero workspaces — CTA to DM the bot (solo) or add it to a group
13. Active sessions table with device label, country hint, per-session revoke, and "log out everywhere else"
14. All auth-related error responses include CORS headers — fixes the pre-existing 500-without-CORS bug on `/api/*` routes

## Non-goals

- WhatsApp and Discord login buttons are rendered as "Bientôt" — not functionally wired in v1
- No UI to actively link a second identity to an existing account — schema supports it, linking flow is v2
- No invitation flow from the web SPA — members are still added by being part of the bot's chat
- No backfill of `workspaces_meta.name` for existing workspaces via the Telegram `getChat` API — admins rename manually in `/w/:slug/settings`
- No syncing of `workspaces_meta.name` back to the Telegram `chat.title` when edited on the web (considered intrusive)
- The Telegram Login Widget does not support local development — dev bypass is provided via a tightly-gated env var

## User journeys

### JA — New Telegram user creates a personal workspace

1. User discovers Grumps via the landing page at `grumps.app`
2. Clicks "Log in with Telegram"
3. Widget popup appears, user confirms — Telegram signs the payload with the bot token
4. SPA posts payload to `POST /auth/telegram/verify`, Worker verifies HMAC, creates a `users` row and `user_identities(platform='telegram', platform_user_id=<tg_id>)` row, issues session cookies
5. `/dashboard` loads, shows empty state: "No workspaces yet. Message @HeyGrumpsBot or add it to a group."
6. User clicks the deep link `tg://resolve?domain=HeyGrumpsBot&start=hello` — Telegram opens the bot DM
7. User sends any message
8. Worker `webhook_telegram::handle_incoming` detects `chat.type == "private"`, provisions a workspace (`is_dm = true`, `name = "Personal"`), upserts the user as admin, replies with the DM welcome message
9. User refreshes `grumps.app/dashboard` — the "Personal" workspace appears

### JB — Admin adds the bot to a group

1. Admin is logged in (already has a user from a prior flow, or logs in via Widget first)
2. Admin adds `@HeyGrumpsBot` to a Telegram group and is promoted to admin (standard TG UX)
3. Bot receives `my_chat_member` with `new_chat_member.status = 'administrator'` and `from` = the admin
4. `handle_first_add` provisions workspace (`name = chat.title`, `is_dm = false`), sets description, sends welcome message to the group, upserts the adding user as workspace admin
5. Refresh `grumps.app/dashboard` — new workspace appears in the list

### JC — Regular group member logs in after being in a Grumps group

1. Another member of the group chats in the group (any text message)
2. Bot `handle_incoming` upserts their `members` row (existing) AND calls `upsert_identity_user` for the TG user — creates their `users` + `user_identities` rows if not present, and links them to the workspace as role `member` via `user_workspaces`
3. They visit `grumps.app` → click "Log in with Telegram" → Widget → session issued
4. Their dashboard shows the group workspace

### JD — Returning user on a new device

1. User already has a Grumps account via Telegram on their laptop
2. Opens `grumps.app` on their phone → landing shows "Log in" CTA (no cookie on phone browser yet)
3. Widget flow → verifies HMAC → lookup by `telegram_user_id` finds existing user → issues a **new session** with a distinct `sid`
4. Both laptop and phone sessions coexist independently
5. From `/settings → Sessions`, user can see both devices and revoke either

## Architecture

### Components

- **Index D1** — schema change (migration 0003): `users` rebuilt to drop `phone`; add `user_identities`, `sessions`; add columns to `workspaces_meta`
- **Worker** — new routes `/auth/telegram/verify`, `/auth/me`, `/auth/logout`, `/auth/sessions`, `/auth/sessions/:id` (DELETE), `/auth/sessions/revoke-all`; middleware reworked to support cookie+CSRF and sessions with KV-cached validity checks; onboarding handlers updated to upsert identity+user_workspaces rows
- **SPA** — new `<AuthGate>` wrapper, new `/dashboard` empty state, new `/settings` page, workspace switcher component in sidebar, `api.rs` refactored to `credentials: 'include'` + CSRF header
- **Landing (`index.html` + `landing/build.mjs`)** — dynamic CTA reads `/auth/me`, new strings `hero.cta.*` in the 14 i18n JSON files
- **i18n locales** — new key `telegram.onboarding.dm_welcome`, new `hero.cta.*` keys, new `auth.*` error keys for toasts

### External dependencies

- **Telegram Login Widget** at `https://telegram.org/js/telegram-widget.js?22` — one-time BotFather setup: `/setdomain` → `grumps.app`
- **Cloudflare KV** (existing binding) — session validity cache, session `last_seen_at` throttle

## Schema migration

File: `migrations/index/0003_identity_first_class.sql`

```sql
BEGIN TRANSACTION;

-- Rebuild users: drop phone (moves to user_identities), add display_name + default_locale.
CREATE TABLE users_new (
  id              TEXT PRIMARY KEY,
  display_name    TEXT,
  default_locale  TEXT,
  created_at      TEXT DEFAULT (datetime('now'))
);

INSERT INTO users_new (id, display_name, default_locale, created_at)
  SELECT id, NULL, NULL, created_at FROM users;

-- Multi-platform identity per user. A given (platform, platform_user_id) pair belongs to exactly one user.
CREATE TABLE user_identities (
  platform         TEXT NOT NULL,
  platform_user_id TEXT NOT NULL,
  user_id          TEXT NOT NULL,
  verified_at      TEXT DEFAULT (datetime('now')),
  PRIMARY KEY (platform, platform_user_id)
);
CREATE INDEX idx_user_identities_user ON user_identities(user_id);

-- Backfill WA users as WhatsApp identities (preserves existing OTP login flow).
INSERT INTO user_identities (platform, platform_user_id, user_id, verified_at)
  SELECT 'whatsapp', phone, id, created_at FROM users;

DROP TABLE users;
ALTER TABLE users_new RENAME TO users;

-- Per-device session registry (for revocation and "log out everywhere").
CREATE TABLE sessions (
  id             TEXT PRIMARY KEY,
  user_id        TEXT NOT NULL,
  user_agent     TEXT,
  device_label   TEXT,
  country_hint   TEXT,
  created_at     TEXT DEFAULT (datetime('now')),
  last_seen_at   TEXT DEFAULT (datetime('now')),
  revoked_at     TEXT NULL
);
CREATE INDEX idx_sessions_user ON sessions(user_id) WHERE revoked_at IS NULL;

-- Workspace metadata: whether it's a DM-only workspace, and whether the bot is still present.
ALTER TABLE workspaces_meta ADD COLUMN is_dm INTEGER NOT NULL DEFAULT 0;
ALTER TABLE workspaces_meta ADD COLUMN archived_at TEXT NULL;

COMMIT;
```

SQLite lacks in-place `ALTER TABLE` for constraint changes, so `users` is rebuilt. The transaction is atomic; no partial state is visible. `user_workspaces.user_id` references `users.id`, preserved through the rebuild.

## Auth endpoints

### `POST /auth/telegram/verify`

Accepts the Widget payload, verifies the HMAC, finds-or-creates a user, issues cookies.

Payload shape (`TelegramWidgetPayload`):

```rust
{
  id: i64,
  first_name: Option<String>,
  last_name: Option<String>,
  username: Option<String>,
  photo_url: Option<String>,
  auth_date: i64,                    // Unix seconds
  hash: String,                      // hex-encoded HMAC-SHA256
  dev_bypass: Option<bool>,          // dev-only shortcut, ignored in prod
}
```

HMAC verification per the [Telegram spec](https://core.telegram.org/widgets/login#checking-authorization):

1. Build `data_check_string` — alphabetically sorted `key=value` lines (excluding `hash`, excluding empty fields), joined by `\n`
2. `secret_key = SHA256(bot_token)` (raw bytes)
3. `computed = HMAC-SHA256(secret_key, data_check_string)`
4. Constant-time compare `hex(computed) == payload.hash`

Additional checks:

- Reject if `now() - auth_date > 3600` seconds (replay protection)
- Reject if any mandatory field missing or malformed

On success: `lookup_user_by_identity("telegram", payload.id)`; if found, reuse user; otherwise create a new user row and identity row. Compute `display_name` as `"{first_name} {last_name}".trim()` with fallback to `username` or `"telegram:{id}"`.

Create a session: generate `sid = uuid::v4()`, parse UA string into `device_label`, read `CF-IPCountry` header for `country_hint`, insert into `sessions`.

Mint JWT with claims `{sub: user_id, sid, csrf: <base64-32-bytes>, workspaces: [...slugs], exp: now+7d}`.

Set cookies:

```
grumps_jwt=<jwt>;  Domain=.grumps.app; Path=/; Max-Age=604800; HttpOnly; Secure; SameSite=Lax
grumps_csrf=<csrf>; Domain=.grumps.app; Path=/; Max-Age=604800;          Secure; SameSite=Lax
```

Return body:

```json
{
  "user_id": "...",
  "display_name": "Benoît M",
  "workspaces": [
    { "slug": "cb5b4bd7", "name": "Colocs", "role": "admin", "platform": "telegram", "is_dm": false },
    { "slug": "...",      "name": "Personal", "role": "admin", "platform": "telegram", "is_dm": true  }
  ],
  "csrf_token": "<base64>"
}
```

### `GET /auth/me`

Session bootstrap — the SPA calls this on mount to hydrate the `SessionContext`. Returns the same shape as `/auth/telegram/verify` (minus the cookies, which are already set). If no session cookie or cookie invalid, returns 401 with `{"error": "auth.unauthenticated"}`.

### `POST /auth/logout`

Revokes the current session (marks `sessions.revoked_at = now()`, invalidates KV cache), expires both cookies with `Max-Age=0`.

### `GET /auth/sessions`

Returns the user's active sessions (where `revoked_at IS NULL`):

```json
{
  "sessions": [
    { "id": "sess_...", "device_label": "Chrome on macOS", "country_hint": "FR",
      "created_at": "...", "last_seen_at": "...", "is_current": true },
    ...
  ]
}
```

The `is_current` flag marks the session whose `sid` is in the requesting cookie — useful for the "This device" badge in the UI.

### `DELETE /auth/sessions/:id`

Revokes a specific session. Returns 404 if the session does not belong to the authenticated user.

### `POST /auth/sessions/revoke-all`

Revokes every session of the user **except** the current one. Returns a count.

### `POST /auth/otp` + `POST /auth/verify` (existing, unchanged)

The WhatsApp OTP flow continues to exist exactly as today. It returns a Bearer JWT in the response body. The SPA doesn't use it in v1 (WA button is "Bientôt"), but the endpoints remain wired for the bot flow and are compatible with the dual-mode middleware below.

## Middleware — cookie+CSRF with Bearer fallback

`crates/worker/src/middleware.rs::verify_session`:

```rust
pub async fn verify_session(req: &Request, env: &Env) -> Result<Claims, AuthError> {
    let secret = env.secret("JWT_SECRET")?.to_string();

    // Preferred: cookie-based session.
    if let Some(jwt) = extract_cookie(req, "grumps_jwt") {
        let claims = decode_jwt(&jwt, &secret)?;

        // Session must exist and not be revoked (KV-cached).
        check_session_active(env, &claims.sid).await?;

        // CSRF check on mutations only.
        if is_mutation_method(req) {
            let header = req.headers().get("X-CSRF-Token").ok().flatten()
                .ok_or(AuthError::CsrfMissing)?;
            if header != claims.csrf {
                return Err(AuthError::CsrfMismatch);
            }
        }

        return Ok(claims);
    }

    // Fallback: legacy Bearer header (WA OTP flow, unchanged).
    let bearer = extract_bearer(req)?;
    decode_jwt(&bearer, &secret).map_err(Into::into)
}
```

`check_session_active`:

```rust
async fn check_session_active(env: &Env, sid: &str) -> Result<(), AuthError> {
    let kv = env.kv("KV")?;
    let cache_key = format!("session:{}", sid);

    if kv.get(&cache_key).text().await?.is_some() {
        return Ok(());                               // cache hit, 1ms
    }

    // Miss: check D1.
    let index_db = get_index_db(env)?;
    let row = index_db
        .prepare("SELECT 1 FROM sessions WHERE id = ?1 AND revoked_at IS NULL")
        .bind(&[sid.into()])?
        .first::<serde_json::Value>(None).await?;

    if row.is_none() {
        return Err(AuthError::SessionRevoked);
    }

    kv.put(&cache_key, "valid")?.expiration_ttl(60).execute().await?;
    Ok(())
}
```

`last_seen_at` is touched via a second KV key (`session:{sid}:last_seen`) with 5 min TTL — skip D1 write if the key exists, otherwise update D1 and repopulate.

## Onboarding gap fix

Three handlers in `webhook_telegram.rs` are updated to also write to `users` + `user_identities` + `user_workspaces`:

- `handle_first_add` — admin who added the bot becomes role `admin` in user_workspaces
- `handle_promotion` — promoted user becomes role `admin`
- `handle_incoming` (new branch for DMs) — if `chat.type == "private"` and no workspace exists for this chat_id, provision a DM workspace and add the user as `admin`; the DM user is also the sole member

For group messages (non-DM), `handle_incoming` additionally performs a lazy upsert: if the sender's TG user_id is not yet in `user_identities`, insert it and link them to the workspace as role `member`. This closes the pre-existing gap for users who were in the group before this feature shipped.

The helper `db::upsert_identity_user` handles the three-table upsert atomically:

```rust
pub async fn upsert_identity_user(
    index_db: &D1Database,
    platform: &str,
    platform_user_id: &str,
    workspace_slug: &str,
    role: &str,
    display_name: Option<&str>,
) -> Result<String /* user_id */> {
    // 1. Try lookup existing identity
    let existing = index_db
        .prepare("SELECT user_id FROM user_identities WHERE platform = ?1 AND platform_user_id = ?2")
        .bind(&[platform.into(), platform_user_id.into()])?
        .first::<UserIdRow>(None).await?;

    let user_id = match existing {
        Some(row) => row.user_id,
        None => {
            // 2. Create new user + identity
            let new_uid = uuid::Uuid::new_v4().to_string();
            index_db.prepare("INSERT INTO users (id, display_name) VALUES (?1, ?2)")
                .bind(&[new_uid.clone().into(), display_name.unwrap_or_default().into()])?
                .run().await?;
            index_db.prepare("INSERT INTO user_identities (platform, platform_user_id, user_id) VALUES (?1, ?2, ?3)")
                .bind(&[platform.into(), platform_user_id.into(), new_uid.clone().into()])?
                .run().await?;
            new_uid
        }
    };

    // 3. Link to workspace (idempotent)
    index_db.prepare(
        "INSERT INTO user_workspaces (user_id, workspace_slug, role) VALUES (?1, ?2, ?3) \
         ON CONFLICT(user_id, workspace_slug) DO NOTHING"
    ).bind(&[user_id.clone().into(), workspace_slug.into(), role.into()])?
     .run().await?;

    Ok(user_id)
}
```

The legacy `upsert_index_user(phone, ...)` is kept as a thin wrapper that calls `upsert_identity_user("whatsapp", phone, ...)`.

## DM workspace provisioning

In `webhook_telegram.rs::handle_incoming`, a new branch detects DMs at the top:

```rust
let is_dm = msg.chat.chat_type == "private";

if is_dm {
    let chat_id = msg.chat.id.to_string();  // equals msg.from.id in DM
    let tg_user_id = msg.from.id.to_string();

    match db::lookup_workspace(&index_db, "telegram", &chat_id).await? {
        Some(_ws) => { /* existing DM workspace, fall through to agent routing */ }
        None => {
            // First contact — provision the DM workspace.
            let locale = Locale::from_code(msg.from.language_code.as_deref().unwrap_or("en"));
            let (slug, _db_id) = provisioning::provision_workspace_dm(
                &d1_client, &index_db, "telegram", &chat_id,
                Some("Personal"),   // default name
                true,               // is_dm
            ).await?;

            db::update_workspace_locale(&index_db, &slug, locale.code()).await?;
            db::upsert_identity_user(
                &index_db, "telegram", &tg_user_id, &slug, "admin",
                Some(&format_tg_display_name(&msg.from)),
            ).await?;

            let welcome = t(locale, "telegram.onboarding.dm_welcome",
                &[("slug", &slug), ("bot", &tg.bot_username)]);
            let _ = send_message(&tg, &chat_id, &OutboundMessage { text: welcome, reply_to: None }).await;

            return Response::ok("ok");   // welcome is the only response on first contact
        }
    }
}
```

`provision_workspace_dm` is a tiny wrapper over `provision_workspace` that passes the DM name and `is_dm = true` into `workspaces_meta`. No schema change beyond what 0003 already does.

## Welcome message variants

New i18n key `telegram.onboarding.dm_welcome` in all 14 locales. English source:

```
Grumps. Your personal workspace is ready: grumps.app/w/{slug}

TODO: <item> — adds a task
NOTE: <text> — pins info
@{bot} help — everything else

Gets it done. No small talk.
```

Shorter than the group variant: no mention of promotion (irrelevant in DM), no hint about group descriptions. The auto-translation pass runs the same way as prior onboarding keys — Sonnet batch + manual review of `fr` and `es`.

No `setChatDescription` call — Telegram DMs have no description field, and the API rejects the call on `chat.type = 'private'`.

## Workspace naming

`workspaces_meta.name` is populated at provisioning time:

- Group: `name = chat.title` from the `my_chat_member` or message payload
- DM: `name = "Personal"` (localized via i18n key `workspace.default_name.personal` so non-EN users get a translated default)

Editable from `/w/:slug/settings` via a new `PATCH /api/w/:slug/settings/name` endpoint (admin-only). The new name is not pushed back to the platform (intrusive).

Display everywhere uses `name` with fallback to `slug`. Affected surfaces: SPA sidebar header, SPA dashboard cards, bot welcome messages (the `slug` in the link stays), bot status messages.

## Landing page

The hero CTA becomes dynamic, driven by a small inline script that fetches `/auth/me`:

```html
<div id="hero-cta">
  <a href="/login" class="btn-primary" data-i18n="hero.cta.login"
     data-i18n-alt-my="hero.cta.my_workspaces"
     data-i18n-alt-start="hero.cta.get_started">Log in</a>
  <a href="#how-it-works" class="btn-secondary" data-i18n="hero.cta.secondary">See how it works</a>
</div>
<script>
  fetch('https://api.grumps.app/auth/me', { credentials: 'include' })
    .then(r => r.ok ? r.json() : null)
    .then(session => {
      if (!session) return;
      const cta = document.querySelector('#hero-cta .btn-primary');
      const key = session.workspaces.length > 0 ? 'alt-my' : 'alt-start';
      cta.textContent = cta.dataset[`i18n${key === 'alt-my' ? 'AltMy' : 'AltStart'}Value`] || cta.textContent;
      cta.href = '/dashboard';
    })
    .catch(() => {});
</script>
```

`landing/build.mjs` resolves the `data-i18n-alt-*` attributes at build time, baking the localized label for the JS to read from `dataset.*Value`. Four new keys:

- `hero.cta.login` — "Log in" / "Se connecter" / etc.
- `hero.cta.my_workspaces` — "My workspaces" / "Mes workspaces" / etc.
- `hero.cta.get_started` — "Get started" / "Commencer" / etc.
- `hero.cta.secondary` — "See how it works" / "Voir comment ça marche" / etc.

The existing "Add Grumps to your group" content stays in the "How it works" section below the hero — it's now part of the onboarding explanation rather than the primary CTA.

## SPA architecture

### Route tree

```rust
<Router base=base>
    <Routes fallback=|| view! { <NotFound/> }>
        <Route path=path!("/login") view=LoginPage/>                  // public
        <AuthGate>
            <Route path=path!("/")          view=RootRedirect/>       // → /dashboard
            <Route path=path!("/dashboard") view=DashboardPage/>
            <Route path=path!("/settings")  view=GlobalSettingsPage/>
            <ParentRoute path=path!("/w/:slug") view=WorkspaceLayout>
                <Route path=path!("/")           view=OverviewPage/>
                <Route path=path!("/todos")      view=TodosPage/>
                <Route path=path!("/notes")      view=NotesPage/>
                <Route path=path!("/notes/:id")  view=NoteEditorPage/>
                <Route path=path!("/files")      view=FilesPage/>
                <Route path=path!("/history")    view=HistoryPage/>
                <Route path=path!("/settings")   view=WorkspaceSettingsPage/>
                <Route path=path!("/memory")     view=MemoryPage/>
                <Route path=path!("/scheduled")  view=ScheduledActionsPage/>
                <Route path=path!("/calendar")   view=CalendarPage/>
                <Route path=path!("/admin/observability") view=ObservabilityPage/>
            </ParentRoute>
        </AuthGate>
        <Route path=path!("/admin/observability") view=GlobalObservabilityPage/>
    </Routes>
</Router>
```

### `<AuthGate>` component

On mount, calls `GET /auth/me` with `credentials: 'include'`. Three states:

- `Loading` — minimal splash (no hero, just the Grumps monogram to avoid flash)
- `Authenticated(SessionContext)` — provides context, renders children
- `Unauthenticated` — `navigate("/login?redirect={current_path}")`

`SessionContext` is provided via `provide_context` and contains `{user_id, display_name, default_locale, workspaces: Vec<WorkspaceRef>, csrf_token}`. It's cloned into any component that needs it via `use_context::<SessionContext>()`.

Demo mode (`crate::demo::is_demo()`) bypasses `AuthGate` entirely — existing demo logic untouched.

### `pages/login.rs`

Brutalist layout, cream base, slab-serif heading:

- Grumps monogram at top
- Telegram Login Widget (script tag rendered inline, reads `data-telegram-login="HeyGrumpsBot"`)
- Two disabled buttons: "Log in with WhatsApp" and "Log in with Discord", opacity 0.5, label "Bientôt"
- Small footer text: "New here? Add @HeyGrumpsBot to a Telegram group."

A global JS callback `window.__grumpsTgAuth(user)` is set by the SPA on mount. When the Widget fires, it calls this function which POSTs the payload to `/auth/telegram/verify`, then navigates to `?redirect=<path>` or `/dashboard`.

### `pages/dashboard.rs`

Reads `SessionContext.workspaces`. Two renders:

- Empty state (workspaces.len() == 0) — centered CTA with two boxes: "Message the bot" (deep link `tg://resolve?domain=HeyGrumpsBot&start=hello`) and "Add to a group" (instructions)
- Populated state — grid of workspace cards:

  ```
  ┌──────────────────────────────┐
  │ Colocs                       │    ← workspaces_meta.name, slab serif
  │ ──────────────────────────   │
  │ TELEGRAM GROUP · 5 members   │    ← derived from (platform, is_dm) + COUNT members
  │                              │
  │ 3 open · 2 done this week    │    ← stats from workspace API
  │                              │
  │                    admin     │    ← user_workspaces.role
  └──────────────────────────────┘
  ```

Cards link to `/w/<slug>`. Below the grid: `+ Add Grumps to another group` card linking to the onboarding instructions.

### `pages/global_settings.rs` (new)

Three sections:

1. **Account** — `display_name` text input (persists to `PATCH /api/me`), `default_locale` select (14 options, persists to same endpoint)
2. **Linked accounts** — lists `user_identities` rows; TG row shows username + ID + `[Unlink]` button (disabled if it's the only identity); WA and Discord rows show "Not linked · Bientôt"
3. **Sessions** — lists active sessions with device_label, country_hint, created_at, last_seen_at, `[This device]` / `[Log out]` per row; `[Log out everywhere else]` button at the bottom

### Workspace switcher

The sidebar top replaces the static `cb5b4bd7 ▼` with a dropdown component reading `SessionContext.workspaces`. Clicking an entry navigates to `/w/<slug>`. A divider + "+ Add to another group" link at the bottom routes back to `/dashboard` with the onboarding CTA.

Platform badge (compact two-letter): `TG GROUP`, `TG DM`, `WA GROUP`, `DC SERVER`.

### API client refactor

`crates/spa/src/api.rs` changes:

- Remove all `localStorage.jwt` reads
- Every `fetch` adds `credentials: 'include'` (cookies sent cross-subdomain)
- Mutations (POST/PUT/PATCH/DELETE) add `X-CSRF-Token: <csrf>` header, read from the `grumps_csrf` cookie via `web_sys::HtmlDocument::cookie()`
- On 401: clear `SessionContext`, `navigate("/login?redirect=" + current_path)`
- On 403 with `error: "auth.csrf_mismatch"`: re-fetch `/auth/me` once to refresh CSRF, retry the original request; if still 403, bubble up

## Cookies + CSRF details

Cookie attributes:

- `Domain=.grumps.app` — shared between `grumps.app` (SPA) and `api.grumps.app` (Worker)
- `Path=/` — all paths
- `Max-Age=604800` — 7 days
- `HttpOnly` on `grumps_jwt` — not readable from JS, protects against XSS
- No `HttpOnly` on `grumps_csrf` — SPA must read it via `document.cookie` to put it in the header
- `Secure` on both — HTTPS only, always true in prod
- `SameSite=Lax` on both — blocks cross-site POST CSRF; allows top-level navigation (needed for future OAuth redirects)

CSRF token:

- 32 random bytes, base64-encoded
- Stored as a JWT claim (`csrf`) AND as the `grumps_csrf` cookie value — triple-match on verify (cookie == header == JWT claim)
- Rotated on every new session (not per-request)

CORS from Worker on `api.grumps.app`:

- `Access-Control-Allow-Origin: <exact origin>` (never `*` with credentials)
- `Access-Control-Allow-Credentials: true`
- `Access-Control-Allow-Headers: Content-Type, Authorization, X-CSRF-Token`
- `Access-Control-Allow-Methods: GET, POST, PUT, PATCH, DELETE, OPTIONS`
- `Access-Control-Max-Age: 86400`

## Error handling

### The fix: all API errors must include CORS headers

Every route's error branch goes through a helper:

```rust
pub fn error_with_cors(req: &Request, status: u16, code: &str, detail: &str) -> Result<Response> {
    let body = serde_json::json!({ "error": code, "detail": detail });
    let mut resp = Response::from_json(&body)?.with_status(status);
    let origin = req.headers().get("Origin")?.unwrap_or_default();
    add_cors(&mut resp, Some(&origin))?;
    Ok(resp)
}
```

Handlers are refactored from `let claims = auth(&req, &ctx)?;` to:

```rust
let claims = match middleware::verify_session(&req, &ctx.env).await {
    Ok(c) => c,
    Err(AuthError::Unauthenticated) => return error_with_cors(&req, 401, "auth.unauthenticated", "session required"),
    Err(AuthError::CsrfMismatch)   => return error_with_cors(&req, 403, "auth.csrf_mismatch", "csrf token mismatch"),
    Err(AuthError::SessionRevoked) => return error_with_cors(&req, 401, "auth.session_revoked", "session no longer valid"),
    Err(e) => return error_with_cors(&req, 401, "auth.invalid_token", &format!("{:?}", e)),
};
```

Applied to all `/api/*` and `/auth/*` handlers — roughly 20 routes.

### SPA error mapping

| HTTP | `error` | SPA reaction |
|---|---|---|
| 401 `auth.unauthenticated` | any | Redirect `/login?redirect=<current>` |
| 401 `auth.expired` / `auth.invalid_token` / `auth.session_revoked` | any | Clear `SessionContext`, redirect `/login` |
| 403 `auth.csrf_mismatch` | mutation | Re-fetch `/auth/me` once, retry; if still fails, toast + manual refresh |
| 403 `auth.not_member` | workspace route | Toast "You're not a member of this workspace", navigate `/dashboard` |
| 401 `auth.invalid_hash` | Widget flow | Toast "Invalid Telegram login, try again" |
| 401 `auth.expired` | Widget flow | Toast "Login expired, try again" |

## Dev bypass

`POST /auth/telegram/verify` accepts a bypass when **three** conditions are met:

1. `ctx.env.var("ENVIRONMENT")` returns a value other than `"production"` (wrangler.toml sets `ENVIRONMENT = "development"` locally)
2. `ctx.env.secret("GRUMPS_DEV_AUTH_BYPASS")` is set and contains a Telegram user_id as its value
3. The request payload includes `"dev_bypass": true`

All three must be present. In prod, even if someone sent `dev_bypass: true`, the env check and missing secret block the shortcut. The bypass skips HMAC verification and uses the configured TG user_id as the identity.

CI verifies the safety invariant:

```rust
#[test]
fn dev_bypass_never_active_in_prod_env() {
    let env = mock_env("ENVIRONMENT", "production");
    set_secret(&env, "GRUMPS_DEV_AUTH_BYPASS", "6108569905");
    let payload = payload_with_dev_bypass();
    let result = handle_telegram_verify(request_with_origin(), env).await;
    assert!(matches!(result, Err(AuthError::InvalidHash)));
}
```

## Workspace archival on bot kick

When `my_chat_member` fires with `new_chat_member.status ∈ {"left", "kicked", "banned"}`, the bot handler sets `workspaces_meta.archived_at = datetime('now')`. The workspace is still readable via the SPA but gets a visual badge "Archived — bot no longer in group". Data is preserved; the user can copy anything out before we offer explicit deletion (v2).

## Testing strategy

### Unit (Rust, `--target x86_64-pc-windows-msvc`)

- `middleware/auth_test.rs` — HMAC verify with TG vectors, tampered payload rejection, expired auth_date rejection, UTF-8 names
- `middleware/cookies_test.rs` — cookie header set/parse round-trip, attribute correctness
- `middleware/csrf_test.rs` — mismatch rejection, GET skip, mutation enforcement
- `middleware/sessions_test.rs` — KV cache hit skips D1, invalidation on revoke
- `auth/telegram_test.rs` — first login creates user, subsequent reuses, workspaces populated correctly, dev bypass gating
- `db/upsert_identity_test.rs` — new user + identity + user_workspaces triple, idempotency on re-call

### Integration (local Worker + local D1)

A script `scripts/test_auth_flow.sh` spins up `wrangler dev` with local D1 and verifies:

- Widget payload with valid HMAC → 200 + cookies set
- Invalid HMAC → 401 with CORS headers
- `/auth/me` without cookie → 401 with CORS
- `/auth/me` with cookie → 200 with workspaces
- Mutation without CSRF → 403
- Mutation with CSRF → 200
- Logout → cookies expired, subsequent `/auth/me` → 401

### E2E (Playwright on prod staging)

Post-deploy smoke test:

1. `grumps.app/login` renders three buttons, TG active, WA/Discord greyed
2. Dev bypass POST to `/auth/telegram/verify` → cookies set, redirect to `/dashboard`
3. Dashboard loads with known workspaces
4. Click a workspace card → `/w/:slug` loads overview with real stats (no 500)
5. Workspace switcher → click another → navigation works
6. `/settings` → "Account" section: change display_name, refresh, persisted
7. `/settings` → "Sessions": current session visible and marked
8. Logout → redirect `/login`, cookies cleared

## Rollout plan

### Order of operations (prod)

1. **Apply migration 0003** manually against `grumps-index` remote D1:

   ```bash
   wrangler d1 execute grumps-index --remote --file=migrations/index/0003_identity_first_class.sql
   ```

   Verify with `SELECT COUNT(*) FROM user_identities` matching the pre-migration users count.

2. **Squash-merge feature branch into `main`** — triggers CI, which deploys Pages (landing + SPA) simultaneously.

3. **Run `wrangler deploy`** to deploy the Worker with the new endpoints and middleware.

4. **Smoke tests** per the E2E checklist above.

The Pages deploy and Worker deploy are independent; if Worker is down briefly, SPA will show 401/network errors and fall back to `/login` gracefully.

### Rollback

- **Worker**: `wrangler rollback` to the previous version (Cloudflare keeps versioned history)
- **SPA**: revert the squash commit on `main` and push; CI redeploys the previous bundle
- **Schema**: a companion file `migrations/index/0003_rollback.sql` is committed (not applied) that recreates the old `users(id, phone UNIQUE NOT NULL, created_at)` table from `user_identities WHERE platform='whatsapp'` and drops the new tables. Untested by default — keep as emergency backup.

### Backward compatibility

The WhatsApp OTP endpoints (`/auth/otp`, `/auth/verify`) remain unchanged and continue to return Bearer tokens. The middleware accepts both cookie+CSRF and Bearer tokens, so no code path is broken.

### Secrets

No new secrets. Existing in place:

- `TG_BOT_TOKEN` — used for Widget HMAC verification
- `JWT_SECRET` — used for session JWT signing

Optional dev-only secret `GRUMPS_DEV_AUTH_BYPASS` — set to a TG user_id in dev environments only; never in prod.

## Open questions (to resolve during implementation)

None at spec time. All design decisions recorded above have explicit user approval. Implementation may surface micro-decisions (exact copy for error toasts, icon choice for platform badges) which stay at the implementer's discretion — no further design review needed.

## References

- [Telegram Login Widget spec](https://core.telegram.org/widgets/login)
- [Widget hash verification algorithm](https://core.telegram.org/widgets/login#checking-authorization)
- Existing flow: `crates/worker/src/routes/auth.rs` (to be extended), `crates/worker/src/middleware.rs` (to be extended)
- Onboarding touchpoint: `crates/worker/src/routes/webhook_telegram.rs::handle_first_add`, `handle_promotion`, `handle_incoming`
