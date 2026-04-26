# Grumps — Working notes for Claude

## Project shape

Group-chat AI agent (WhatsApp / Telegram / Discord) with a workspace
SPA. Rust+WASM on Cloudflare Workers, D1 + KV + Vectorize. Anthropic
Sonnet + Gemini Flash cascade. See `SPECS.md` for the full spec and
`docs/superpowers/` for designs and historical plans.

## Internationalization (REQUIRED)

Grumps ships in **14 languages**:

`en` · `es` · `pt-BR` · `fr` · `de` · `it` · `ru` · `tr` · `ar` (RTL) · `hi` · `zh-CN` · `ja` · `ko` · `id`

**Hard rule: no user-facing text in source code.** Every string visible
to a user — landing page, SPA, bot messages, error toasts, email
templates, OG tags, push notifications — MUST go through the i18n layer.
If you find yourself typing a sentence directly into a `.rs`, `.html`,
or template file, stop and add a key to the appropriate dictionary
instead.

### Where strings live

A single `crates/i18n/` crate is the source of truth for both worker
and SPA. Flat-key JSON dictionaries (no Fluent) — keeps the SPA WASM
bundle small and the API trivial.

| Surface | Format | Source files |
|---|---|---|
| Landing page | JSON | `landing/strings.{lang}.json` |
| SPA + bot/worker | JSON via `grumps-i18n` | `crates/i18n/locales/{lang}.json` |
| LLM-generated text | n/a — system prompt injects the target locale; the LLM replies in the user's locale |

### When adding a feature

1. Add the English key first to the appropriate JSON.
2. Reference it via `t(locale, "key", &[("var", "value")])` (worker)
   or `tr(key)` / `tr_p(key, params)` / `tr_n(key_base, n, params)` (SPA helpers).
   For the landing, use `data-i18n="key"` on the element.
3. Run `scripts/i18n-translate.sh KEY` (TODO: build) to fill the 13
   other locales via Sonnet batch. Manually review `fr` and `es`;
   ship the rest as-is.
4. Test `ar` visually for RTL layout.
5. Test `tr` visually — Turkish phrases run ~15-20% longer than EN and
   often expose tight `width:` constraints. Prefer `min-width` over
   fixed widths on buttons, tabs, badges.

### Pluralization

**Never use `"{n} message(s)"` or `if n == 1 { "" } else { "s" }`** —
it works for English only and reads as broken in every other language.

Use the `t_plural()` / `tr_n()` helper which picks `key_base.{plural-category}`
based on the locale's CLDR rule. Plural categories supported per locale:

| Locale family | Categories |
|---|---|
| en, de, es, pt-BR, it, tr, hi, id | `one` (n=1), `other` |
| fr | `one` (n=0 or 1), `other` |
| ru | `one`, `few` (2-4), `many` (5+) |
| ar | `zero`, `one`, `two`, `few` (3-10), `many` (11-99), `other` |
| zh-CN, ja, ko | `other` (no plural distinction) |

Example dictionary entries:

```json
"schedule.fired.one":   "Fired {n} time",
"schedule.fired.other": "Fired {n} times",
"schedule.fired.few":   "Запущено {n} раза",
"schedule.fired.many":  "Запущено {n} раз"
```

`tr_n("schedule.fired", n, &[])` automatically picks the right key and
injects `{n}`. The `t_plural()` fallback chain is per-category-key →
`.other` → English → literal key, so missing translations degrade
gracefully but visibly.

### Locale detection

- **Landing**: `Accept-Language` → CF Pages redirect → `/{lang}/`
  (fallback `en`). Lang switcher in footer.
- **SPA**: `?lang=` query > `localStorage.locale` > `Accept-Language` > `en`
- **Bot**: per-member `members.locale` column. Set on first message
  from the language detected by the Gemini classifier (free win — it
  already returns a language hint). Group-level default fallback, then
  `en`.

### RTL

Only `ar` is RTL. SPA: layout uses logical CSS properties
(`margin-inline-*`, `padding-inline-*`, `inset-inline-*`) where
possible. Landing: ships a small RTL stylesheet variant loaded
conditionally.

### CJK + Devanagari fonts

`Bitter` (display) and `DM Sans` (body) don't cover Chinese, Japanese,
Korean, Hindi, or Arabic glyphs. Each non-Latin locale loads a Noto
fallback in its `<head>`:
- `zh-CN`: Noto Sans SC + Noto Serif SC
- `ja`: Noto Sans JP + Noto Serif JP
- `ko`: Noto Sans KR + Noto Serif KR
- `hi`: Noto Sans Devanagari + Noto Serif Devanagari
- `ar`: Noto Sans Arabic + Noto Serif Arabic

The build script (`landing/build.mjs`) picks the right `<link>` per
locale variant.

## Demo mode

The landing page embeds the actual SPA in an iframe rather than a
hand-maintained HTML mockup. When the SPA is loaded under `/demo/...`
or with `?demo=1`, every API call is short-circuited to deterministic
seed data and the auth gate is bypassed. See `crates/spa/src/demo.rs`.

Build the SPA bundle into `crates/spa/dist/` with:

```bash
cd crates/spa
mkdir -p dist
tailwindcss -i ./input.css -o ./dist/styles.css --minify
MSYS_NO_PATHCONV=1 trunk build --release --public-url /demo/
```

The landing build (`node landing/build.mjs`) detects `crates/spa/dist/`
and copies it into `dist/demo/`, then rewrites the iframe src to
`/demo/?lang={code}` so the iframe locale matches the surrounding
landing.

## Design system

Warm brutalism. Slab serif (`Bitter`) for headings, `DM Sans` for body.
Cream `#F5F0E8` base + brick `#C0392B` for urgency, teal `#1B6B5A` for
success, ochre `#D4940A` for warnings. 2px ink borders. No shadows
beyond the offset-block style. Subtle grain overlay (SVG noise).
Voice: terse and dry — "Done." not "Successfully completed!" — no
emojis in UI chrome. Reference: `index.html` and `crates/spa/`.

## Git workflow

Linear history. Feature branches use **gitmoji** format for atomic
commits (one emoji + one-line capitalised subject, e.g.
`✨ Add PATCH workspace locale endpoint`). Feature-branch commits
bypass the commitizen hook with:

```bash
SKIP=commitizen git commit --no-verify -m "✨ <Subject>"
```

Squash-merge to main produces **one commit per feature** in
conventional-commits format (`type(scope): subject` — e.g.
`feat(telegram): add localised onboarding UX`). The commitizen
hook accepts this without `--no-verify`.

Commit messages MUST NOT include `Co-Authored-By:` trailers and
MUST NOT reference internal plan identifiers (e.g. "Plan A",
"Plan B"); use generic descriptive language instead.

Reference: `.claude/commands/gitmoji.md`,
`.claude/commands/conventional.md`.

## Rust toolchain

Tests: build with `--target x86_64-pc-windows-msvc`. Use rustup's
`cargo` (`~/.cargo/bin/cargo.exe`) — do not pick up the Chocolatey
shim. Recommended invocation:

```bash
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe test \
  --target x86_64-pc-windows-msvc
```

## D1 migrations

Two separate migration directories, applied to different databases.

- `migrations/index/` — applied to the global index DB (tables:
  `users`, `workspaces_meta`, `user_workspaces`). **No runtime
  mechanism.** Apply manually: `wrangler d1 execute <index-db>
  --file=migrations/index/<file>.sql`.
- `migrations/workspace/` — applied to every new per-workspace D1 at
  provisioning time via `crates/worker/src/provisioning.rs`. Existing
  workspace DBs must be migrated out-of-band (same wrangler command,
  looped over every workspace).

### Conventions

- File name: `NNNN_snake_case.sql` — 4 digits, strictly sequential,
  never reuse or renumber. One logical change per file. Leading
  comment explains the "why".
- `ALTER TABLE … ADD COLUMN` prefers `NOT NULL DEFAULT '<x>'` so
  existing rows populate automatically. Only fall back to NULL if
  there's no sensible default.
- **Workspace migrations only**: after creating the `.sql` file, also
  add the `include_str!` constant AND the matching `exec_statements`
  call in `provisioning.rs`. Forgetting either breaks new-workspace
  provisioning silently.
- Before deployment: apply to the production index DB and, for
  workspace migrations, loop over every existing workspace DB. Never
  rely on next-write-fixes-it.

## Admin model

Two tiers:
- **Workspace admin** — manages a single group's settings and members.
  Determined by the `members.role = 'admin'` row for the authenticated
  user in the workspace's D1 (`is_workspace_admin` /
  `is_workspace_admin_by_slug` in `middleware.rs`). The first member to
  interact with the bot in a new workspace is auto-promoted to admin
  (DM provisioning + group `chat_member` flow).
- **Super admin** — the project operator. Sole access to the
  cross-workspace observability dashboard, cost analytics, and global
  admin tools. Identification details below.

Never expose super-admin surfaces to workspace admins — they're
distinct roles. Always gate super-admin endpoints with
`is_super_admin(env, claims)`, never trust a header or query param.

### How super-admin identity is verified

The Telegram Login Widget does **not** expose phone numbers, only the
stable Telegram numeric user id, first/last name, and username (signed
by the bot token). Identity flow:

1. **Login**: `POST /auth/widget` receives the widget payload. We HMAC-
   verify it against `TG_BOT_TOKEN` (`verify_widget_hash`,
   constant-time compare) and reject `auth_date` older than 1 h.
2. **Identity persistence**: a row in `user_identities` maps
   `(platform="telegram", platform_user_id=<tg id>) → user_id`. First
   login mints a new user_id; subsequent logins look it up.
3. **JWT**: `create_jwt_with_csrf` encodes `sub=user_id`,
   `tg_user_id=<tg id>`, `sid=<session id>`, `csrf=<token>`. Signed
   with `JWT_SECRET`, 7-day exp, set as `HttpOnly; Secure;
   SameSite=Lax; Domain=.grumps.app` cookie.
4. **Each request**: `verify_session` decodes the cookie JWT, checks
   `sid` is still active (D1 `sessions` table, KV-cached 60 s) and —
   on mutating methods — that the `X-CSRF-Token` header matches the
   `csrf` claim.
5. **Super-admin gate**: `is_super_admin(env, claims)` reads
   `SUPER_ADMIN_TELEGRAM_IDS` (comma-separated Telegram numeric ids)
   via `env.secret()` and compares with constant-prefix matching
   against `claims.tg_user_id`. Falls back to `SUPER_ADMIN_PHONES` /
   `claims.phone` for the legacy WA OTP path; that path is deprecated.

Why we don't use phone for the gate: the Widget never exposes it, so
`claims.phone` is empty for everyone who logged in via Telegram. We
also avoid putting any super-admin identifier in `wrangler.toml`
`[vars]` because that file is committed; secrets stay in
`wrangler secret put` (prod) or `.dev.vars` (gitignored, local only).

### Webhook authenticity

- Telegram: every `POST /webhook/telegram` MUST present the
  `X-Telegram-Bot-Api-Secret-Token` header matching `TG_WEBHOOK_SECRET`.
  The header is mandatory — missing header = 500. (Earlier code treated
  absence as "skip verification"; that bypass has been closed.)
- WhatsApp: same model with `X-Hub-Signature-256` HMAC against
  `WA_APP_SECRET`. Header is mandatory.
- KV-based dedup on `message_id` blocks replay within the 24 h TTL
  window even if the secret were ever leaked and rotated.

### Workspace isolation

Two D1 databases. Index DB (one) holds users, workspaces, sessions,
identities. Per-workspace DB (one per group) holds chat-bound data:
todos, notes, members, scheduled actions, agent sessions. Every
workspace-scoped route runs `check_workspace_access(claims, slug)`
which checks `claims.workspaces.contains(slug)`. The workspace list
is fixed at login time and refreshed on `/auth/me`.

### Secrets handling

- `wrangler.toml` is committed. `[vars]` block is fine for non-secret
  configuration; never put credentials there.
- `.dev.vars` is gitignored. Local-only. Production secrets are set
  via `wrangler secret put`.
- `.gitignore` excludes `.env*`, `.dev.vars*`, `*.rlib`, `target/`,
  `dist/`, `node_modules/`, `.wrangler/`, `.worktrees/`. Verify
  before adding a new file at root.

### What MUST NOT land in committed files

This is a public repo. Treat every commit as world-readable and
permanent.

- No real phone numbers, real Telegram ids, real email addresses, or
  real personal names in source, tests, fixtures, comments, or
  config. Use placeholders (`+10000000000`, `1234567890`,
  `user@example.com`, `"Test User"`).
- No production API keys, bot tokens, webhook secrets, JWT signing
  keys, database ids that grant access — use `wrangler secret put`.
- No internal plan identifiers (`Plan A`, `Plan B`, etc.) in
  commit messages or code comments — see commit-message rule.
- No screenshots showing real phone numbers, real names, or real
  group titles at the repo root or in `docs/`.
