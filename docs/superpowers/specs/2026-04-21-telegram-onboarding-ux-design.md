# Telegram Onboarding UX — Design

## Goal

Replace the hardcoded-English welcome flow at `crates/worker/src/routes/webhook_telegram.rs:153` with a localised, state-aware onboarding that:

1. Greets the group in the user's language (14 supported locales).
2. Instructs the group admin to promote the bot to admin when relevant. Admin promotion is the single gate that unlocks (a) writing the workspace link into the group description via `setChatDescription` and (b) receiving all group messages for ambient features (privacy-mode-independent).
3. Confirms when the promotion actually takes effect, updating the description at that moment.
4. Exposes a manual workspace-locale override in the SPA settings page.

## Non-goals

- No retry or reminder loop if the user ignores the "promote me" CTA (would be nagging).
- No reaction to bot demotion — the current silent behaviour is preserved.
- No media handling — tracked separately under `telegram-media-handling`.
- No equivalent changes to WhatsApp or Discord. Their adapters receive TODO markers pointing to this spec.
- No command-based locale override from chat — locale is changed from the SPA only.

## Architecture decision

Privacy mode stays **ON globally** at the BotFather level (default). The promotion of the bot to group admin is what unlocks both description writes and ambient message reception for that specific group. This is more privacy-respecting than the alternative (global privacy-mode off) because each group explicitly opts in, and it collapses two capabilities to a single trigger for the user.

Benoît must verify once via `/setprivacy` in BotFather that privacy mode is ON (it is by default). See the DEPLOY.md update below.

## Flow states

The `my_chat_member` webhook event gives both `old_chat_member.status` and `new_chat_member.status`. Detection uses this pair alone; no persistent state is required.

| Trigger | Detected via `(old → new)` | Action |
|---|---|---|
| First add, normal member | `left` / absent → `member` | Provision workspace if needed. Send **V2** (welcome + promotion CTA). |
| First add, promoted immediately | `left` / absent → `administrator` | Provision workspace if needed. Call `setChatDescription`. Send **V1** (short welcome). |
| Promotion after the fact | `member` → `administrator` | Call `setChatDescription`. Send **V3** (variant depends on API result). |
| Re-add after removal | `kicked` / `left` → `member` / `administrator` | Same as first-add branches. Workspace already exists; `provision_workspace` must be idempotent (lookup-then-insert). |
| Demotion (`administrator` → `member` / `left` / `kicked`) | any | Ignored. No message. |

This table becomes the routing logic in the `my_chat_member` handler. `handle_bot_added` is split into `handle_first_add` and `handle_promotion`.

## Message content (English source of truth)

All five strings are atomic keys — one full message per key — rather than composed from fragments. Translators reorder clauses freely per locale (important for JA, AR, DE).

### `telegram.onboarding.welcome.added_as_admin` (V1)

```
Grumps. Your workspace: grumps.io/w/{slug}

TODO: <item> — adds a task
NOTE: <text> — pins info
@{bot} help — everything else

Gets it done. No small talk.
```

### `telegram.onboarding.welcome.added_as_member` (V2)

```
Grumps. Your workspace: grumps.io/w/{slug}

TODO: <item> — adds a task
NOTE: <text> — pins info
@{bot} help — everything else

Promote me to admin for the workspace link in the description and ambient features. Group → Administrators → @{bot}.

Gets it done. No small talk.
```

### `telegram.onboarding.promoted.with_description` (V3a)

```
Admin. Description updated.
```

### `telegram.onboarding.promoted.without_description` (V3b)

```
Admin.
```

### `telegram.onboarding.description`

```
Grumps workspace: grumps.io/w/{slug}
Gets it done. No small talk.
```

### Translator rules

A header comment in `crates/i18n/locales/en.json` (which serves as the source dictionary) states:

- Never translate `TODO:` or `NOTE:` — these are literal command tokens matched by the parser.
- Never translate `@{bot}` — the bot username is invariant.
- `{slug}` and `{bot}` are placeholders substituted at runtime.

## Locale detection

Resolution order at `handle_my_chat_member`:

1. Read `my_chat_member.from.language_code` (requires adding `language_code: Option<String>` to `TgUser` in `crates/messaging/src/telegram.rs`).
2. Pass through `grumps_i18n::normalize_locale(raw: &str) -> &'static str`, new helper:
   - `en-US`, `en-GB`, `en` → `en`
   - `pt-BR`, `pt-PT`, `pt` → `pt-BR`
   - `zh-CN`, `zh-Hans`, `zh` → `zh-CN`
   - Any ISO 639-1 code matching one of the 14 supported → that code
   - Anything else (including `""` and absent) → `en`
3. Persist on the workspace row: `UPDATE workspaces_meta SET locale = ?1 WHERE slug = ?2`.
4. Pass the resolved locale to every subsequent `t()` call for this interaction (welcome message, description).

## i18n key layout

Flat keys under the `telegram.onboarding.*` namespace. Five new keys (above). All 14 locale JSONs (`crates/i18n/locales/{lang}.json`) receive them via the existing batch translation pipeline described in `CLAUDE.md`. French and Spanish reviewed manually; other 12 shipped as-is.

## Data model changes

### New migration: `migrations/index/0002_workspace_locale.sql`

```sql
-- Workspace-level locale, resolved from the bot adder's Telegram
-- language_code at first add. Used as the default for system-level
-- bot messages in the group when no member-specific locale applies.
ALTER TABLE workspaces_meta ADD COLUMN locale TEXT NOT NULL DEFAULT 'en';
```

Applied manually to production per the D1 migration convention in `CLAUDE.md`. No `provisioning.rs` changes needed — this is an index-DB migration, not a workspace-DB migration.

### TgUser struct change

```rust
#[derive(Debug, Deserialize)]
pub struct TgUser {
    pub id: i64,
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub is_bot: Option<bool>,
    pub language_code: Option<String>,  // new
}
```

### TgChatMemberUpdated struct change

Add `old_chat_member: TgChatMember` so transition detection has both sides:

```rust
#[derive(Debug, Deserialize)]
pub struct TgChatMemberUpdated {
    pub chat: TgChat,
    pub from: TgUser,
    pub old_chat_member: TgChatMember,  // new
    pub new_chat_member: TgChatMember,
}
```

## Code changes

| Path | Change |
|---|---|
| `crates/messaging/src/telegram.rs` | Add `language_code` on `TgUser`, `old_chat_member` on `TgChatMemberUpdated`. |
| `crates/i18n/src/lib.rs` | Add `normalize_locale(&str) -> &'static str`. Load 5 new keys. |
| `crates/i18n/locales/*.json` | Add 5 new `telegram.onboarding.*` keys across all 14 files. |
| `crates/worker/src/routes/webhook_telegram.rs` | Refactor `my_chat_member` branch into a transition router (4 cases per table). Split `handle_bot_added` into `handle_first_add` + `handle_promotion`. Remove hardcoded English strings. Look up workspace locale and pass to `t()`. Parse `setChatDescription` response to pick V3a vs V3b. |
| `crates/worker/src/db.rs` | Add `update_workspace_locale(slug, locale)` and update existing workspace lookup to include the `locale` column. |
| `migrations/index/0002_workspace_locale.sql` | New file. |
| `crates/messaging/src/whatsapp.rs` | One-line TODO comment at the top: onboarding/admin flow to be aligned with `telegram-onboarding-ux` spec. |
| `crates/messaging/src/discord.rs` | Same. |
| `crates/agent/src/loop_.rs` `build_prompt_context` (line ~237) | Replace the hardcoded `language: "fr".to_string()` at line 273 with a read of `workspaces_meta.locale`, using the member's locale (`members.locale` from migration 0009) when a specific member is targeted. |
| `DEPLOY.md` step 10 | Add privacy-mode verification note (see below). |

## SPA workspace locale picker

### UI change

Replace the placeholder row at `crates/spa/src/pages/settings.rs:83`:

```rust
<SettingRow label_key="settings.row.language" value="English".to_string() />
```

with a functional `<select>` mirroring the persona picker at lines 100-108 — 2-px ink border, cream background, native names as option labels, current value read from the workspace settings load. `on:change` fires the API call and shows a terse toast (`settings.locale.updated` = "Saved.").

### API endpoint

`PATCH /api/workspace/settings/locale`

Request:
```json
{ "locale": "fr" }
```

- **Auth**: workspace admin (`role == "admin"`), existing auth middleware pattern.
- **Validation**: `locale` must be one of the 14 supported codes. Otherwise 400.
- **Action**: `UPDATE workspaces_meta SET locale = ?1 WHERE slug = ?2`.
- **Side effect**: re-trigger `setChatDescription` on the Telegram chat with the description rendered in the new locale. The chat ID is read from `workspaces_meta.platform_channel_id`; the adapter is rebuilt from env secrets. Best-effort, silent failure (same policy as V3 routing). No user feedback if it fails.
- **Response**: `{ "ok": true, "locale": "fr" }`.

### Scope limits

Changing workspace locale does **not**:
- Re-send past messages in the new language.
- Override per-member locales learned by the Gemini classifier.
- Clear or modify locale-specific content already rendered in the SPA.

## Error handling

- `setChatDescription` response parsed as `{ ok: bool, ... }`. `ok == true` → V3a. `ok == false` or network error → V3b. Full body logged via `console_log!("setChatDescription failed: {body}")`. Never surfaced to the user.
- `sendMessage` failure (welcome or V3): log and return `Response::ok("ok")` to Telegram. We must not let Telegram retry the webhook, which would re-deliver the same `my_chat_member` event and cause duplicate welcome messages.
- `language_code` present but normalised to unsupported → silent fallback to `en`. No log.
- `provision_workspace` called for a pre-existing workspace (re-add case) must not duplicate rows. Plan-level task: verify or add `ON CONFLICT DO NOTHING` / lookup-then-insert.

## Testing

### `crates/messaging/src/telegram.rs`

Three new `parse_webhook` tests for the `my_chat_member` path:
- `left → member` with `language_code: "fr"` — asserts locale extracted, status detected.
- `left → administrator` — asserts status detected.
- `member → administrator` — asserts promotion detected.

### `crates/i18n/src/lib.rs`

Unit tests for `normalize_locale` covering:
- Direct match (`fr`, `ja`).
- Regional variant collapse (`en-US` → `en`, `en-GB` → `en`).
- `pt-PT` → `pt-BR` (we only ship Brazilian Portuguese).
- `zh-Hans` → `zh-CN`.
- Fallback (`hr` → `en`, `""` → `en`).

### `crates/worker/src/routes/webhook_telegram.rs`

The transition router is extracted as a pure function `fn route_chat_member(old: &str, new: &str) -> Transition` returning an enum of `{FirstAddAsMember, FirstAddAsAdmin, Promotion, Ignore}`. Unit-tested exhaustively. The async handlers consume the `Transition` and dispatch.

No integration test against the Telegram API — consistent with the existing pattern (outbound `Fetch::Request` calls are not mocked anywhere).

### SPA settings

If the SPA has a test harness for settings (to verify at plan time), add a test for the locale dropdown: initial render reflects the stored workspace locale, `on:change` triggers the correct PATCH with the new code. Otherwise, covered by manual smoke test in dev.

## DEPLOY.md update (step 10)

Add a new sub-step before 10.1:

> **10.0. Privacy mode check (one-time per bot)**
>
> In a DM with [@BotFather](https://t.me/BotFather), send `/setprivacy`, pick `@grumps_bot`, and verify the current state is **Enabled**. Privacy mode is ON by default on new bots; this is just a verification step.
>
> Grumps relies on the group-admin-promotes-the-bot workflow to unlock access to non-mention messages. Disabling privacy mode globally would work but removes the per-group opt-in — do not do this.
>
> If you toggle privacy mode after the bot is already in a group, Telegram does not apply the change retroactively. You must remove the bot from the group and re-add it.

## Risks

- **Existing workspaces do not receive a retroactive welcome.** Acceptable — re-sending would be intrusive and confusing.
- **Manual migration step required.** Applying `0002_workspace_locale.sql` to production index DB is a deployment checklist item. Missing it crashes at first workspace lookup because the column is `NOT NULL` and existing rows would violate the `SELECT locale` expectation. Mitigation: the migration uses `DEFAULT 'en'`, so existing rows are populated automatically at ALTER time — safe.
- **`from.language_code` absent on old Telegram clients.** Mitigated by fallback to `en`. Users on very old clients get English welcome but can override via the SPA picker.
- **Multi-locale groups.** A French-speaking admin adds the bot in a mixed FR/EN/ES group. Workspace locale becomes `fr` — wrong for EN/ES members' system messages. Mitigation: per-member locales are still learned by the Gemini classifier on first message and take precedence over workspace locale for any message directed at that specific member. Workspace locale is only the default.
