# Telegram Onboarding UX Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the hardcoded-English welcome flow for Telegram with a localised, state-aware onboarding that reacts correctly to promotion events and exposes a workspace-locale picker in the SPA.

**Architecture:** Privacy mode stays ON globally at the BotFather level. Group admin promoting the bot is the single trigger that unlocks both `setChatDescription` writes and ambient message reception. The `my_chat_member` event carries both `old_chat_member.status` and `new_chat_member.status`, enabling stateless detection of first-add / promotion transitions. Workspace locale is resolved from `my_chat_member.from.language_code` at first add, persisted in a new `workspaces_meta.locale` column, and overridable via a SPA settings picker.

**Tech Stack:** Rust (Cloudflare Workers + workers-rs), Cloudflare D1, Leptos (SPA), `grumps_i18n` for localisation. No new external dependencies.

**Spec reference:** `docs/superpowers/specs/2026-04-21-telegram-onboarding-ux-design.md`

---

## File Structure

**Files to create:**
- `migrations/index/0002_workspace_locale.sql` — index DB ALTER for new locale column.

**Files to modify:**
- `crates/messaging/src/telegram.rs` — struct additions (`language_code`, `old_chat_member`) + extended tests.
- `crates/worker/src/routes/webhook_telegram.rs` — transition router, localised dispatch, parsed `setChatDescription` response.
- `crates/worker/src/db.rs` — `WorkspaceMetaRow.locale`, `update_workspace_locale`, SELECT column list.
- `crates/worker/src/routes/workspace_api.rs` — new PATCH handler for locale.
- `crates/worker/src/lib.rs` — register new PATCH route.
- `crates/agent/src/loop_.rs` — read workspace locale instead of hardcoded `"fr"`.
- `crates/messaging/src/whatsapp.rs` — TODO header comment.
- `crates/messaging/src/discord.rs` — TODO header comment.
- `crates/i18n/locales/{en,es,pt-BR,fr,de,it,ru,tr,ar,hi,zh-CN,ja,ko,id}.json` — 6 new keys.
- `crates/spa/src/pages/settings.rs` — replace placeholder language row.
- `crates/spa/src/api.rs` — new PATCH client method.
- `DEPLOY.md` — insert step 10.0 (privacy mode verification) + index-migration instructions.

**Reused without modification:** `crates/i18n/src/lib.rs` — `Locale::from_code` already implements the normalisation required by the spec.

---

## Task 1: D1 migration + DEPLOY.md privacy note

**Files:**
- Create: `migrations/index/0002_workspace_locale.sql`
- Modify: `DEPLOY.md`

- [ ] **Step 1: Create the migration file**

Write `migrations/index/0002_workspace_locale.sql`:

```sql
-- Workspace-level locale, resolved from the bot adder's Telegram
-- language_code at first add. Used as the default for system-level
-- bot messages in the group when no member-specific locale applies.
-- NOT NULL DEFAULT 'en' so existing workspaces_meta rows populate
-- automatically at ALTER time.
ALTER TABLE workspaces_meta ADD COLUMN locale TEXT NOT NULL DEFAULT 'en';
```

- [ ] **Step 2: Update `DEPLOY.md` step 10 with privacy verification**

Find the heading `## Step 10: Telegram Setup (Optional)` and insert a new subsection **before** `### 10.1 Create a Bot`:

```markdown
### 10.0 Privacy mode check (one-time per bot)

In a DM with [@BotFather](https://t.me/BotFather), send `/setprivacy`,
pick `@grumps_bot`, and verify the current state is **Enabled**.
Privacy mode is ON by default on new bots; this is just a verification.

Grumps relies on the group-admin-promotes-the-bot workflow to unlock
non-mention message reception. Disabling privacy mode globally would
work but removes the per-group opt-in — do not do this.

If you toggle privacy mode after the bot is already in a group,
Telegram does not apply the change retroactively. Remove the bot
from the group and re-add it.
```

- [ ] **Step 3: Add a deploy-time migration reminder**

In `DEPLOY.md`, find the `## Migrations` section (or the end of the deploy flow if no such section exists — search for `wrangler d1 execute`). Append:

```markdown
### Index DB migration: workspace locale

Apply before first deploy of the Telegram onboarding UX change:

```bash
wrangler d1 execute grumps-index --file=migrations/index/0002_workspace_locale.sql --remote
```
```

(Replace `grumps-index` with the actual D1 binding name from `wrangler.toml` `[[d1_databases]]` entry for `INDEX_DB` — check before running.)

- [ ] **Step 4: Commit**

```bash
git add migrations/index/0002_workspace_locale.sql DEPLOY.md
git commit -m "feat(db): add workspaces_meta.locale migration + privacy note"
```

---

## Task 2: Add `language_code` to `TgUser`

**Files:**
- Modify: `crates/messaging/src/telegram.rs`

- [ ] **Step 1: Write the failing test**

In the `#[cfg(test)] mod tests` block of `crates/messaging/src/telegram.rs`, add:

```rust
#[test]
fn parse_user_with_language_code() {
    let payload = serde_json::json!({
        "update_id": 1,
        "message": {
            "message_id": 42,
            "from": {"id": 123, "first_name": "Alice", "is_bot": false, "language_code": "fr"},
            "chat": {"id": -100123, "type": "group", "title": "Roommates"},
            "date": 1713000000,
            "text": "hello"
        }
    });
    let update: TgUpdate = serde_json::from_value(payload).unwrap();
    let user = update.message.unwrap().from.unwrap();
    assert_eq!(user.language_code.as_deref(), Some("fr"));
}
```

- [ ] **Step 2: Run the test — verify it fails**

```bash
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe test --target x86_64-pc-windows-msvc -p grumps-messaging telegram::tests::parse_user_with_language_code
```

Expected: FAIL — compile error (`no field language_code on TgUser`) or runtime assertion.

- [ ] **Step 3: Add the field**

In `crates/messaging/src/telegram.rs`, find the `TgUser` struct and add `language_code`:

```rust
#[derive(Debug, Deserialize)]
pub struct TgUser {
    pub id: i64,
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub is_bot: Option<bool>,
    pub language_code: Option<String>,
}
```

- [ ] **Step 4: Run the test — verify it passes**

```bash
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe test --target x86_64-pc-windows-msvc -p grumps-messaging telegram::tests::parse_user_with_language_code
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/messaging/src/telegram.rs
git commit -m "feat(telegram): parse language_code from user objects"
```

---

## Task 3: Add `old_chat_member` to `TgChatMemberUpdated`

**Files:**
- Modify: `crates/messaging/src/telegram.rs`

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block:

```rust
#[test]
fn parse_my_chat_member_with_transition() {
    let payload = serde_json::json!({
        "update_id": 11,
        "my_chat_member": {
            "chat": {"id": -100999, "type": "group", "title": "Test"},
            "from": {"id": 123, "first_name": "Alice", "is_bot": false, "language_code": "de"},
            "old_chat_member": {
                "status": "member",
                "user": {"id": 888, "first_name": "Grumps", "is_bot": true}
            },
            "new_chat_member": {
                "status": "administrator",
                "user": {"id": 888, "first_name": "Grumps", "is_bot": true}
            }
        }
    });
    let update: TgUpdate = serde_json::from_value(payload).unwrap();
    let mcm = update.my_chat_member.unwrap();
    assert_eq!(mcm.old_chat_member.status, "member");
    assert_eq!(mcm.new_chat_member.status, "administrator");
    assert_eq!(mcm.from.language_code.as_deref(), Some("de"));
}
```

- [ ] **Step 2: Run the test — verify it fails**

```bash
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe test --target x86_64-pc-windows-msvc -p grumps-messaging telegram::tests::parse_my_chat_member_with_transition
```

Expected: FAIL — `no field old_chat_member` or missing field on deserialise.

- [ ] **Step 3: Add the field**

Modify `TgChatMemberUpdated`:

```rust
#[derive(Debug, Deserialize)]
pub struct TgChatMemberUpdated {
    pub chat: TgChat,
    pub from: TgUser,
    pub old_chat_member: TgChatMember,
    pub new_chat_member: TgChatMember,
}
```

- [ ] **Step 4: Run the test — verify it passes**

```bash
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe test --target x86_64-pc-windows-msvc -p grumps-messaging telegram::tests::parse_my_chat_member_with_transition
```

Expected: PASS.

- [ ] **Step 5: Update the existing `parse_bot_added_event` test**

The pre-existing `parse_bot_added_event` test (in the same `mod tests`) does not include `old_chat_member`, which now breaks deserialisation. Add it to the test payload:

```rust
#[test]
fn parse_bot_added_event() {
    let payload = serde_json::json!({
        "update_id": 10,
        "my_chat_member": {
            "chat": {"id": -100999, "type": "group", "title": "Test Group"},
            "from": {"id": 123, "first_name": "Alice", "is_bot": false},
            "old_chat_member": {
                "status": "left",
                "user": {"id": 888, "first_name": "Grumps", "is_bot": true}
            },
            "new_chat_member": {
                "status": "member",
                "user": {"id": 888, "first_name": "Grumps", "is_bot": true}
            }
        }
    });
    let update: TgUpdate = serde_json::from_value(payload).unwrap();
    assert!(update.my_chat_member.is_some());
    assert!(update.message.is_none());
    let member = update.my_chat_member.unwrap();
    assert_eq!(member.new_chat_member.status, "member");
    assert_eq!(member.old_chat_member.status, "left");
    assert_eq!(member.chat.id, -100999);
    assert_eq!(member.chat.title, Some("Test Group".into()));
}
```

- [ ] **Step 6: Run all telegram tests — verify all pass**

```bash
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe test --target x86_64-pc-windows-msvc -p grumps-messaging telegram
```

Expected: PASS (including the updated `parse_bot_added_event`).

- [ ] **Step 7: Commit**

```bash
git add crates/messaging/src/telegram.rs
git commit -m "feat(telegram): parse old_chat_member for status transitions"
```

---

## Task 4: Transition router pure function + tests

**Files:**
- Modify: `crates/worker/src/routes/webhook_telegram.rs`

- [ ] **Step 1: Write failing tests**

At the bottom of `crates/worker/src/routes/webhook_telegram.rs`, add a new `#[cfg(test)] mod tests` block (or extend an existing one):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_first_add_as_member() {
        assert_eq!(route_chat_member("left", "member"), Transition::FirstAddAsMember);
        assert_eq!(route_chat_member("kicked", "member"), Transition::FirstAddAsMember);
    }

    #[test]
    fn routes_first_add_as_admin() {
        assert_eq!(route_chat_member("left", "administrator"), Transition::FirstAddAsAdmin);
        assert_eq!(route_chat_member("kicked", "administrator"), Transition::FirstAddAsAdmin);
    }

    #[test]
    fn routes_promotion() {
        assert_eq!(route_chat_member("member", "administrator"), Transition::Promotion);
        assert_eq!(route_chat_member("restricted", "administrator"), Transition::Promotion);
    }

    #[test]
    fn ignores_demotion() {
        assert_eq!(route_chat_member("administrator", "member"), Transition::Ignore);
        assert_eq!(route_chat_member("administrator", "left"), Transition::Ignore);
        assert_eq!(route_chat_member("administrator", "kicked"), Transition::Ignore);
    }

    #[test]
    fn ignores_noop_and_unknown() {
        assert_eq!(route_chat_member("member", "member"), Transition::Ignore);
        assert_eq!(route_chat_member("administrator", "administrator"), Transition::Ignore);
        assert_eq!(route_chat_member("garbage", "member"), Transition::FirstAddAsMember);
        assert_eq!(route_chat_member("", "administrator"), Transition::FirstAddAsAdmin);
    }
}
```

- [ ] **Step 2: Run tests — verify they fail**

```bash
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe test --target x86_64-pc-windows-msvc -p grumps-worker webhook_telegram::tests
```

Expected: FAIL — compile error, `Transition` and `route_chat_member` undefined.

- [ ] **Step 3: Implement the enum + function**

Near the top of `crates/worker/src/routes/webhook_telegram.rs` (after the `use` block, before `pub async fn handle_incoming`), add:

```rust
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Transition {
    FirstAddAsMember,
    FirstAddAsAdmin,
    Promotion,
    Ignore,
}

/// Classify a bot `my_chat_member` status transition. Pure: no I/O, easy to test.
/// Unknown `old` statuses (empty, "left", "kicked", anything not admin) are
/// treated as "bot wasn't in the group before", so a `new == member` or
/// `new == administrator` counts as a first-add.
pub(crate) fn route_chat_member(old: &str, new: &str) -> Transition {
    let was_in_group_as_non_admin = matches!(old, "member" | "restricted");
    let was_admin = old == "administrator";
    match (was_admin, was_in_group_as_non_admin, new) {
        (_, _, "administrator") if was_in_group_as_non_admin => Transition::Promotion,
        (false, false, "administrator") => Transition::FirstAddAsAdmin,
        (false, false, "member") => Transition::FirstAddAsMember,
        _ => Transition::Ignore,
    }
}
```

- [ ] **Step 4: Run tests — verify they pass**

```bash
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe test --target x86_64-pc-windows-msvc -p grumps-worker webhook_telegram::tests
```

Expected: PASS (all 5 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/worker/src/routes/webhook_telegram.rs
git commit -m "feat(telegram): add transition router for my_chat_member events"
```

---

## Task 5: `locale` column in `db.rs`

**Files:**
- Modify: `crates/worker/src/db.rs`

- [ ] **Step 1: Extend `WorkspaceMetaRow`**

In `crates/worker/src/db.rs`, find the `WorkspaceMetaRow` struct and add `locale`:

```rust
#[derive(Deserialize, Debug, Clone)]
pub struct WorkspaceMetaRow {
    pub slug: String,
    pub d1_database_id: String,
    pub name: Option<String>,
    pub plan: String,
    pub locale: String,
}
```

- [ ] **Step 2: Update both `SELECT` queries to include `locale`**

In `lookup_workspace_by_slug`:

```rust
pub async fn lookup_workspace_by_slug(index_db: &D1Database, slug: &str) -> Result<Option<WorkspaceMetaRow>> {
    index_db.prepare("SELECT slug, d1_database_id, name, plan, locale FROM workspaces_meta WHERE slug = ?1")
        .bind(&[slug.into()])?.first::<WorkspaceMetaRow>(None).await
}
```

In `lookup_workspace`:

```rust
pub async fn lookup_workspace(index_db: &D1Database, platform: &str, channel_id: &str) -> Result<Option<WorkspaceMetaRow>> {
    index_db.prepare("SELECT slug, d1_database_id, name, plan, locale FROM workspaces_meta WHERE platform = ?1 AND platform_channel_id = ?2")
        .bind(&[platform.into(), channel_id.into()])?.first::<WorkspaceMetaRow>(None).await
}
```

- [ ] **Step 3: Add `update_workspace_locale` function**

Just after `lookup_workspace` (before the `upsert_index_user` function) add:

```rust
/// Update the locale column on workspaces_meta for the given slug.
/// Caller is responsible for validating that `locale` is a supported code.
pub async fn update_workspace_locale(index_db: &D1Database, slug: &str, locale: &str) -> Result<()> {
    index_db.prepare("UPDATE workspaces_meta SET locale = ?1 WHERE slug = ?2")
        .bind(&[locale.into(), slug.into()])?
        .run().await?;
    Ok(())
}
```

- [ ] **Step 4: Fix any `WorkspaceMetaRow` constructors**

In `crates/worker/src/routes/webhook_telegram.rs`, find the line in `handle_incoming` that constructs a `WorkspaceMetaRow` manually after provisioning:

```rust
Some(ws) => ws,
None => {
    let (slug, db_id) = provisioning::provision_workspace(&d1_client, &index_db, "telegram", &inbound.channel_id).await?;
    db::WorkspaceMetaRow { slug, d1_database_id: db_id, name: None, plan: "free".into() }
}
```

Change the construction to include locale (resolved by Task 7 — for now, stub with `"en"`):

```rust
Some(ws) => ws,
None => {
    let (slug, db_id) = provisioning::provision_workspace(&d1_client, &index_db, "telegram", &inbound.channel_id).await?;
    db::WorkspaceMetaRow { slug, d1_database_id: db_id, name: None, plan: "free".into(), locale: "en".into() }
}
```

Task 7 will replace the `"en"` stub with the resolved locale.

- [ ] **Step 5: Compile check**

```bash
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe check --target x86_64-pc-windows-msvc -p grumps-worker
```

Expected: builds clean. Any site that constructs `WorkspaceMetaRow { ... }` manually must now supply `locale`; fix each by adding `locale: "en".into()`.

- [ ] **Step 6: Commit**

```bash
git add crates/worker/src/db.rs crates/worker/src/routes/webhook_telegram.rs
git commit -m "feat(db): read/write workspaces_meta.locale column"
```

---

## Task 6: Add 6 new i18n keys across all 14 locales

**Files:**
- Modify: `crates/i18n/locales/{14 files}.json`

**Context:** Six keys to add:
- `telegram.onboarding.welcome.added_as_admin` (V1)
- `telegram.onboarding.welcome.added_as_member` (V2)
- `telegram.onboarding.promoted.with_description` (V3a)
- `telegram.onboarding.promoted.without_description` (V3b)
- `telegram.onboarding.description`
- `settings.locale.updated` (SPA toast)

The JSON files are flat key → string maps (see `crates/i18n/src/lib.rs:105-119`). Append the keys to each file. Newlines inside strings use `\n` escapes.

- [ ] **Step 1: Add translator-rules header + English source to `en.json`**

Open `crates/i18n/locales/en.json`. At the **top** of the file, add a JSON comment is not legal — instead, add a `_TRANSLATOR_NOTES` key as the first entry (ignored by the runtime since no lookup uses it, and serves as documentation):

```json
"_TRANSLATOR_NOTES": "Do not translate: 'TODO:' and 'NOTE:' (literal command tokens). '@{bot}' is the invariant bot username. '{slug}' and '{bot}' are runtime placeholders.",
```

Then add the six onboarding strings anywhere in the object (alphabetical order recommended):

```json
"settings.locale.updated": "Saved.",
"telegram.onboarding.welcome.added_as_admin": "Grumps. Your workspace: grumps.io/w/{slug}\n\nTODO: <item> — adds a task\nNOTE: <text> — pins info\n@{bot} help — everything else\n\nGets it done. No small talk.",
"telegram.onboarding.welcome.added_as_member": "Grumps. Your workspace: grumps.io/w/{slug}\n\nTODO: <item> — adds a task\nNOTE: <text> — pins info\n@{bot} help — everything else\n\nPromote me to admin for the workspace link in the description and ambient features. Group → Administrators → @{bot}.\n\nGets it done. No small talk.",
"telegram.onboarding.promoted.with_description": "Admin. Description updated.",
"telegram.onboarding.promoted.without_description": "Admin.",
"telegram.onboarding.description": "Grumps workspace: grumps.io/w/{slug}\nGets it done. No small talk."
```

- [ ] **Step 2: Add the keys to `fr.json`**

```json
"settings.locale.updated": "Enregistré.",
"telegram.onboarding.welcome.added_as_admin": "Grumps. Votre workspace : grumps.io/w/{slug}\n\nTODO: <élément> — ajoute une tâche\nNOTE: <texte> — épingle une info\n@{bot} help — tout le reste\n\nÇa avance. Pas de blabla.",
"telegram.onboarding.welcome.added_as_member": "Grumps. Votre workspace : grumps.io/w/{slug}\n\nTODO: <élément> — ajoute une tâche\nNOTE: <texte> — épingle une info\n@{bot} help — tout le reste\n\nPromeus-moi admin pour que le lien workspace soit dans la description et pour débloquer les fonctions ambient. Groupe → Administrateurs → @{bot}.\n\nÇa avance. Pas de blabla.",
"telegram.onboarding.promoted.with_description": "Admin. Description mise à jour.",
"telegram.onboarding.promoted.without_description": "Admin.",
"telegram.onboarding.description": "Workspace Grumps : grumps.io/w/{slug}\nÇa avance. Pas de blabla."
```

- [ ] **Step 3: Add to `es.json`**

```json
"settings.locale.updated": "Guardado.",
"telegram.onboarding.welcome.added_as_admin": "Grumps. Tu espacio: grumps.io/w/{slug}\n\nTODO: <ítem> — añade una tarea\nNOTE: <texto> — fija una nota\n@{bot} help — todo lo demás\n\nHecho. Sin cháchara.",
"telegram.onboarding.welcome.added_as_member": "Grumps. Tu espacio: grumps.io/w/{slug}\n\nTODO: <ítem> — añade una tarea\nNOTE: <texto> — fija una nota\n@{bot} help — todo lo demás\n\nHazme admin para poner el enlace del workspace en la descripción y activar las funciones ambient. Grupo → Administradores → @{bot}.\n\nHecho. Sin cháchara.",
"telegram.onboarding.promoted.with_description": "Admin. Descripción actualizada.",
"telegram.onboarding.promoted.without_description": "Admin.",
"telegram.onboarding.description": "Espacio Grumps: grumps.io/w/{slug}\nHecho. Sin cháchara."
```

- [ ] **Step 4: Add to `pt-BR.json`**

```json
"settings.locale.updated": "Salvo.",
"telegram.onboarding.welcome.added_as_admin": "Grumps. Seu workspace: grumps.io/w/{slug}\n\nTODO: <item> — cria uma tarefa\nNOTE: <texto> — fixa uma nota\n@{bot} help — o resto\n\nResolve. Sem papo furado.",
"telegram.onboarding.welcome.added_as_member": "Grumps. Seu workspace: grumps.io/w/{slug}\n\nTODO: <item> — cria uma tarefa\nNOTE: <texto> — fixa uma nota\n@{bot} help — o resto\n\nMe promova a admin para o link do workspace entrar na descrição e ativar os recursos ambient. Grupo → Administradores → @{bot}.\n\nResolve. Sem papo furado.",
"telegram.onboarding.promoted.with_description": "Admin. Descrição atualizada.",
"telegram.onboarding.promoted.without_description": "Admin.",
"telegram.onboarding.description": "Workspace Grumps: grumps.io/w/{slug}\nResolve. Sem papo furado."
```

- [ ] **Step 5: Add to `de.json`**

```json
"settings.locale.updated": "Gespeichert.",
"telegram.onboarding.welcome.added_as_admin": "Grumps. Dein Workspace: grumps.io/w/{slug}\n\nTODO: <Eintrag> — legt eine Aufgabe an\nNOTE: <Text> — merkt sich eine Info\n@{bot} help — alles Weitere\n\nErledigt. Kein Smalltalk.",
"telegram.onboarding.welcome.added_as_member": "Grumps. Dein Workspace: grumps.io/w/{slug}\n\nTODO: <Eintrag> — legt eine Aufgabe an\nNOTE: <Text> — merkt sich eine Info\n@{bot} help — alles Weitere\n\nMach mich zum Admin — dann kommt der Workspace-Link in die Gruppenbeschreibung und die Ambient-Funktionen werden freigeschaltet. Gruppe → Administratoren → @{bot}.\n\nErledigt. Kein Smalltalk.",
"telegram.onboarding.promoted.with_description": "Admin. Beschreibung aktualisiert.",
"telegram.onboarding.promoted.without_description": "Admin.",
"telegram.onboarding.description": "Grumps-Workspace: grumps.io/w/{slug}\nErledigt. Kein Smalltalk."
```

- [ ] **Step 6: Add to `it.json`**

```json
"settings.locale.updated": "Salvato.",
"telegram.onboarding.welcome.added_as_admin": "Grumps. Il tuo workspace: grumps.io/w/{slug}\n\nTODO: <voce> — aggiunge un'attività\nNOTE: <testo> — fissa un'info\n@{bot} help — tutto il resto\n\nSi fa. Niente chiacchiere.",
"telegram.onboarding.welcome.added_as_member": "Grumps. Il tuo workspace: grumps.io/w/{slug}\n\nTODO: <voce> — aggiunge un'attività\nNOTE: <testo> — fissa un'info\n@{bot} help — tutto il resto\n\nRendimi admin per mettere il link del workspace nella descrizione e attivare le funzioni ambient. Gruppo → Amministratori → @{bot}.\n\nSi fa. Niente chiacchiere.",
"telegram.onboarding.promoted.with_description": "Admin. Descrizione aggiornata.",
"telegram.onboarding.promoted.without_description": "Admin.",
"telegram.onboarding.description": "Workspace Grumps: grumps.io/w/{slug}\nSi fa. Niente chiacchiere."
```

- [ ] **Step 7: Add to `ru.json`**

```json
"settings.locale.updated": "Сохранено.",
"telegram.onboarding.welcome.added_as_admin": "Grumps. Ваше пространство: grumps.io/w/{slug}\n\nTODO: <пункт> — добавляет задачу\nNOTE: <текст> — закрепляет заметку\n@{bot} help — всё остальное\n\nДелает. Без болтовни.",
"telegram.onboarding.welcome.added_as_member": "Grumps. Ваше пространство: grumps.io/w/{slug}\n\nTODO: <пункт> — добавляет задачу\nNOTE: <текст> — закрепляет заметку\n@{bot} help — всё остальное\n\nНазначьте меня администратором, чтобы ссылка на пространство появилась в описании группы и заработали ambient-функции. Группа → Администраторы → @{bot}.\n\nДелает. Без болтовни.",
"telegram.onboarding.promoted.with_description": "Админ. Описание обновлено.",
"telegram.onboarding.promoted.without_description": "Админ.",
"telegram.onboarding.description": "Пространство Grumps: grumps.io/w/{slug}\nДелает. Без болтовни."
```

- [ ] **Step 8: Add to `tr.json`**

```json
"settings.locale.updated": "Kaydedildi.",
"telegram.onboarding.welcome.added_as_admin": "Grumps. Çalışma alanınız: grumps.io/w/{slug}\n\nTODO: <öğe> — görev ekler\nNOTE: <metin> — bilgi sabitler\n@{bot} help — geri kalan her şey\n\nİşi bitirir. Laf yok.",
"telegram.onboarding.welcome.added_as_member": "Grumps. Çalışma alanınız: grumps.io/w/{slug}\n\nTODO: <öğe> — görev ekler\nNOTE: <metin> — bilgi sabitler\n@{bot} help — geri kalan her şey\n\nBeni admin yap — çalışma alanı linki açıklamaya eklensin ve ambient özellikler açılsın. Grup → Yöneticiler → @{bot}.\n\nİşi bitirir. Laf yok.",
"telegram.onboarding.promoted.with_description": "Yönetici. Açıklama güncellendi.",
"telegram.onboarding.promoted.without_description": "Yönetici.",
"telegram.onboarding.description": "Grumps çalışma alanı: grumps.io/w/{slug}\nİşi bitirir. Laf yok."
```

- [ ] **Step 9: Add to `ar.json`**

```json
"settings.locale.updated": "تم الحفظ.",
"telegram.onboarding.welcome.added_as_admin": "Grumps. مساحة عملك: grumps.io/w/{slug}\n\nTODO: <عنصر> — يضيف مهمة\nNOTE: <نص> — يثبّت ملاحظة\n@{bot} help — كل شيء آخر\n\nينجز. بلا ثرثرة.",
"telegram.onboarding.welcome.added_as_member": "Grumps. مساحة عملك: grumps.io/w/{slug}\n\nTODO: <عنصر> — يضيف مهمة\nNOTE: <نص> — يثبّت ملاحظة\n@{bot} help — كل شيء آخر\n\nعيّنني مشرفًا لإضافة رابط مساحة العمل إلى الوصف وتفعيل ميزات ambient. المجموعة ← المشرفون ← @{bot}.\n\nينجز. بلا ثرثرة.",
"telegram.onboarding.promoted.with_description": "مشرف. تحدّث الوصف.",
"telegram.onboarding.promoted.without_description": "مشرف.",
"telegram.onboarding.description": "مساحة عمل Grumps: grumps.io/w/{slug}\nينجز. بلا ثرثرة."
```

- [ ] **Step 10: Add to `hi.json`**

```json
"settings.locale.updated": "सहेजा गया।",
"telegram.onboarding.welcome.added_as_admin": "Grumps। आपका वर्कस्पेस: grumps.io/w/{slug}\n\nTODO: <आइटम> — कार्य जोड़ता है\nNOTE: <टेक्स्ट> — जानकारी पिन करता है\n@{bot} help — बाकी सब\n\nकाम करता है। बकवास नहीं।",
"telegram.onboarding.welcome.added_as_member": "Grumps। आपका वर्कस्पेस: grumps.io/w/{slug}\n\nTODO: <आइटम> — कार्य जोड़ता है\nNOTE: <टेक्स्ट> — जानकारी पिन करता है\n@{bot} help — बाकी सब\n\nमुझे एडमिन बनाएँ ताकि वर्कस्पेस लिंक डिस्क्रिप्शन में जुड़े और ambient फ़ीचर चालू हो जाएँ। ग्रुप → एडमिन → @{bot}।\n\nकाम करता है। बकवास नहीं।",
"telegram.onboarding.promoted.with_description": "एडमिन। डिस्क्रिप्शन अपडेट हो गया।",
"telegram.onboarding.promoted.without_description": "एडमिन।",
"telegram.onboarding.description": "Grumps वर्कस्पेस: grumps.io/w/{slug}\nकाम करता है। बकवास नहीं।"
```

- [ ] **Step 11: Add to `zh-CN.json`**

```json
"settings.locale.updated": "已保存。",
"telegram.onboarding.welcome.added_as_admin": "Grumps。你的工作区：grumps.io/w/{slug}\n\nTODO: <内容> — 添加任务\nNOTE: <文本> — 钉住信息\n@{bot} help — 其他一切\n\n把事做完。不废话。",
"telegram.onboarding.welcome.added_as_member": "Grumps。你的工作区：grumps.io/w/{slug}\n\nTODO: <内容> — 添加任务\nNOTE: <文本> — 钉住信息\n@{bot} help — 其他一切\n\n把我设为管理员 — 工作区链接会加入群组描述，ambient 功能也会启用。群组 → 管理员 → @{bot}。\n\n把事做完。不废话。",
"telegram.onboarding.promoted.with_description": "管理员。描述已更新。",
"telegram.onboarding.promoted.without_description": "管理员。",
"telegram.onboarding.description": "Grumps 工作区：grumps.io/w/{slug}\n把事做完。不废话。"
```

- [ ] **Step 12: Add to `ja.json`**

```json
"settings.locale.updated": "保存しました。",
"telegram.onboarding.welcome.added_as_admin": "Grumps。ワークスペース: grumps.io/w/{slug}\n\nTODO: <項目> — タスクを追加\nNOTE: <テキスト> — 情報をピン留め\n@{bot} help — その他全部\n\nやり遂げる。雑談なし。",
"telegram.onboarding.welcome.added_as_member": "Grumps。ワークスペース: grumps.io/w/{slug}\n\nTODO: <項目> — タスクを追加\nNOTE: <テキスト> — 情報をピン留め\n@{bot} help — その他全部\n\n管理者に昇格してください — ワークスペースのリンクが説明に入り、ambient 機能が有効になります。グループ → 管理者 → @{bot}。\n\nやり遂げる。雑談なし。",
"telegram.onboarding.promoted.with_description": "管理者。説明を更新しました。",
"telegram.onboarding.promoted.without_description": "管理者。",
"telegram.onboarding.description": "Grumps ワークスペース: grumps.io/w/{slug}\nやり遂げる。雑談なし。"
```

- [ ] **Step 13: Add to `ko.json`**

```json
"settings.locale.updated": "저장됨.",
"telegram.onboarding.welcome.added_as_admin": "Grumps. 워크스페이스: grumps.io/w/{slug}\n\nTODO: <항목> — 작업 추가\nNOTE: <내용> — 정보 고정\n@{bot} help — 그 외 전부\n\n해냅니다. 잡담 없음.",
"telegram.onboarding.welcome.added_as_member": "Grumps. 워크스페이스: grumps.io/w/{slug}\n\nTODO: <항목> — 작업 추가\nNOTE: <내용> — 정보 고정\n@{bot} help — 그 외 전부\n\n관리자로 승격해 주세요. 워크스페이스 링크가 그룹 설명에 추가되고 ambient 기능이 켜집니다. 그룹 → 관리자 → @{bot}.\n\n해냅니다. 잡담 없음.",
"telegram.onboarding.promoted.with_description": "관리자. 설명이 업데이트되었습니다.",
"telegram.onboarding.promoted.without_description": "관리자.",
"telegram.onboarding.description": "Grumps 워크스페이스: grumps.io/w/{slug}\n해냅니다. 잡담 없음."
```

- [ ] **Step 14: Add to `id.json`**

```json
"settings.locale.updated": "Tersimpan.",
"telegram.onboarding.welcome.added_as_admin": "Grumps. Workspace-mu: grumps.io/w/{slug}\n\nTODO: <item> — menambah tugas\nNOTE: <teks> — menyematkan info\n@{bot} help — sisanya\n\nSelesai. Tanpa basa-basi.",
"telegram.onboarding.welcome.added_as_member": "Grumps. Workspace-mu: grumps.io/w/{slug}\n\nTODO: <item> — menambah tugas\nNOTE: <teks> — menyematkan info\n@{bot} help — sisanya\n\nJadikan saya admin supaya link workspace masuk ke deskripsi dan fitur ambient aktif. Grup → Administrator → @{bot}.\n\nSelesai. Tanpa basa-basi.",
"telegram.onboarding.promoted.with_description": "Admin. Deskripsi diperbarui.",
"telegram.onboarding.promoted.without_description": "Admin.",
"telegram.onboarding.description": "Workspace Grumps: grumps.io/w/{slug}\nSelesai. Tanpa basa-basi."
```

- [ ] **Step 15: Verify the JSON is valid in every file**

```bash
for f in crates/i18n/locales/*.json; do
  python -m json.tool "$f" > /dev/null && echo "OK: $f" || echo "FAIL: $f"
done
```

Expected: every file says `OK`. If any fail, fix trailing commas / stray characters before proceeding.

- [ ] **Step 16: Run i18n tests (if any) — verify nothing broken**

```bash
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe test --target x86_64-pc-windows-msvc -p grumps-i18n
```

Expected: PASS (or "no tests", which is fine).

- [ ] **Step 17: Commit**

```bash
git add crates/i18n/locales/
git commit -m "i18n(telegram): add onboarding keys across 14 locales"
```

---

## Task 7: Refactor webhook_telegram handler

**Files:**
- Modify: `crates/worker/src/routes/webhook_telegram.rs`

**Context:** This is the core refactor. The handler at `crates/worker/src/routes/webhook_telegram.rs:8` currently uses `raw.pointer()` to poke into the JSON, then calls a single `handle_bot_added` regardless of transition. We replace this with:
1. Typed deserialisation of `my_chat_member` into `TgChatMemberUpdated`.
2. Routing via `route_chat_member`.
3. Separate handlers per transition.
4. Locale lookup using `Locale::from_code(language_code).code()`.
5. Localised messages via `grumps_i18n::t()`.
6. Parsed `setChatDescription` response to pick V3a vs V3b.

- [ ] **Step 1: Add i18n import**

At the top of `crates/worker/src/routes/webhook_telegram.rs`, alongside the existing `use` declarations, add:

```rust
use grumps_i18n::{t, Locale};
use grumps_messaging::telegram::{TgUpdate, TgChatMemberUpdated};
```

Remove `use grumps_messaging::telegram::TelegramAdapter;` if the import needs adjusting — the concrete path may be `grumps_messaging::telegram::TelegramAdapter`. Follow the existing import style of the file.

- [ ] **Step 2: Replace the `my_chat_member` branch in `handle_incoming`**

Replace the block currently at lines ~20-34 (`if let Some(member_update) = raw.get("my_chat_member")` through the end of `handle_bot_added` call):

```rust
    // Typed parse of my_chat_member (if present) — routes based on status transition.
    if let Ok(update) = serde_json::from_slice::<TgUpdate>(&body) {
        if let Some(mcm) = update.my_chat_member {
            let transition = route_chat_member(
                &mcm.old_chat_member.status,
                &mcm.new_chat_member.status,
            );
            return match transition {
                Transition::FirstAddAsMember => handle_first_add(&ctx, &tg, &mcm, false).await,
                Transition::FirstAddAsAdmin => handle_first_add(&ctx, &tg, &mcm, true).await,
                Transition::Promotion => handle_promotion(&ctx, &tg, &mcm).await,
                Transition::Ignore => Response::ok("ok"),
            };
        }
    }
```

(If the existing code also re-parses as `raw: serde_json::Value` for other reasons, keep that parse — just remove the `my_chat_member` special-case block that referenced `raw.pointer(...)`.)

- [ ] **Step 3: Delete the old `handle_bot_added` and add `handle_first_add`**

Remove the entire `async fn handle_bot_added(...)` function (currently lines ~153-207 of the file) and replace with:

```rust
async fn handle_first_add(
    ctx: &RouteContext<()>,
    tg: &TelegramAdapter,
    mcm: &TgChatMemberUpdated,
    added_as_admin: bool,
) -> Result<Response> {
    let index_db = db::get_index_db(&ctx.env)?;
    let d1_client = D1RestClient::from_env(&ctx.env)?;

    let chat_id = mcm.chat.id.to_string();
    let locale = Locale::from_code(
        mcm.from.language_code.as_deref().unwrap_or("en")
    );

    // Provision workspace (idempotent via lookup-then-insert).
    let slug = match db::lookup_workspace(&index_db, "telegram", &chat_id).await? {
        Some(ws) => ws.slug,
        None => {
            let (slug, _db_id) = provisioning::provision_workspace(
                &d1_client, &index_db, "telegram", &chat_id,
            ).await?;
            slug
        }
    };

    // Persist the resolved locale on the workspace row.
    let _ = db::update_workspace_locale(&index_db, &slug, locale.code()).await;

    // If added as admin, set description immediately.
    if added_as_admin {
        let description = t(locale, "telegram.onboarding.description", &[("slug", &slug)]);
        let _ = call_set_description(tg, &chat_id, &description).await;
    }

    // Render + send the welcome message.
    let welcome_key = if added_as_admin {
        "telegram.onboarding.welcome.added_as_admin"
    } else {
        "telegram.onboarding.welcome.added_as_member"
    };
    let welcome = t(locale, welcome_key, &[("slug", &slug), ("bot", &tg.bot_username)]);

    let msg = grumps_messaging::adapter::OutboundMessage { text: welcome, reply_to: None };
    let _ = send_message(tg, &chat_id, &msg).await;

    Response::ok("ok")
}
```

- [ ] **Step 4: Add `handle_promotion`**

Append immediately after `handle_first_add`:

```rust
async fn handle_promotion(
    ctx: &RouteContext<()>,
    tg: &TelegramAdapter,
    mcm: &TgChatMemberUpdated,
) -> Result<Response> {
    let index_db = db::get_index_db(&ctx.env)?;
    let chat_id = mcm.chat.id.to_string();

    // Look up the workspace — it must exist (user was already in group before promotion).
    // If for some reason it doesn't, fall through silently.
    let ws = match db::lookup_workspace(&index_db, "telegram", &chat_id).await? {
        Some(ws) => ws,
        None => return Response::ok("ok"),
    };
    let locale = Locale::from_code(&ws.locale);

    // Re-trigger setChatDescription in the workspace locale.
    let description = t(locale, "telegram.onboarding.description", &[("slug", &ws.slug)]);
    let description_ok = call_set_description(tg, &chat_id, &description).await
        .unwrap_or(false);

    // V3 picks variant based on whether setChatDescription actually succeeded.
    let key = if description_ok {
        "telegram.onboarding.promoted.with_description"
    } else {
        "telegram.onboarding.promoted.without_description"
    };
    let text = t(locale, key, &[]);

    let msg = grumps_messaging::adapter::OutboundMessage { text, reply_to: None };
    let _ = send_message(tg, &chat_id, &msg).await;

    Response::ok("ok")
}
```

- [ ] **Step 5: Add the two helper functions for HTTP calls**

Append at the bottom of the file (after `build_adapter`):

```rust
/// Call `setChatDescription`, parse the response `{ ok: bool }`. Returns
/// `Ok(true)` if Telegram returned `ok: true`, `Ok(false)` on any other
/// response or non-2xx, `Err` only on local failure (request build, etc.).
async fn call_set_description(
    tg: &TelegramAdapter,
    chat_id: &str,
    description: &str,
) -> Result<bool> {
    let (url, body) = tg.build_set_description_request(chat_id, description)
        .map_err(|e| Error::RustError(format!("{:?}", e)))?;
    let mut headers = Headers::new();
    headers.set("Content-Type", "application/json")?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post).with_headers(headers).with_body(Some(body.into()));
    let req = Request::new_with_init(&url, &init)?;
    let mut resp = match Fetch::Request(req).send().await {
        Ok(r) => r,
        Err(e) => {
            worker::console_log!("setChatDescription network error: {}", e);
            return Ok(false);
        }
    };
    match resp.json::<serde_json::Value>().await {
        Ok(v) => {
            let ok = v.get("ok").and_then(|b| b.as_bool()).unwrap_or(false);
            if !ok {
                worker::console_log!("setChatDescription failed: {}", v);
            }
            Ok(ok)
        }
        Err(e) => {
            worker::console_log!("setChatDescription: response not JSON: {}", e);
            Ok(false)
        }
    }
}

/// Send a text message to Telegram. Logs on failure but does not propagate,
/// so the webhook always returns 200 OK (preventing Telegram retry storms).
async fn send_message(
    tg: &TelegramAdapter,
    chat_id: &str,
    msg: &grumps_messaging::adapter::OutboundMessage,
) -> Result<()> {
    let (url, body) = tg.build_send_request(chat_id, msg)
        .map_err(|e| Error::RustError(format!("{:?}", e)))?;
    let mut headers = Headers::new();
    headers.set("Content-Type", "application/json")?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post).with_headers(headers).with_body(Some(body.into()));
    let req = Request::new_with_init(&url, &init)?;
    if let Err(e) = Fetch::Request(req).send().await {
        worker::console_log!("sendMessage failed: {}", e);
    }
    Ok(())
}
```

- [ ] **Step 6: Update the stub in `handle_incoming` to use resolved locale**

In `handle_incoming`, the `WorkspaceMetaRow` construction from Task 5 step 4 currently stubs `locale: "en".into()`. Since provisioning here happens on a first *message* (not a bot-add event), we don't have `language_code` from `from` — but we do from the inbound message. Update to:

```rust
Some(ws) => ws,
None => {
    // First time we're seeing a message from this channel without a prior
    // my_chat_member event — unusual but possible (e.g. Telegram bug, or
    // bot was added before this code was deployed). Provision with 'en'
    // as fallback; admin can override via SPA.
    let (slug, db_id) = provisioning::provision_workspace(&d1_client, &index_db, "telegram", &inbound.channel_id).await?;
    db::WorkspaceMetaRow { slug, d1_database_id: db_id, name: None, plan: "free".into(), locale: "en".into() }
}
```

Keep the stub as `"en"` — that's the correct fallback for the rare case.

- [ ] **Step 7: Compile**

```bash
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe check --target x86_64-pc-windows-msvc -p grumps-worker
```

Expected: builds clean.

- [ ] **Step 8: Run all webhook_telegram tests**

```bash
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe test --target x86_64-pc-windows-msvc -p grumps-worker webhook_telegram
```

Expected: existing + transition router tests all pass.

- [ ] **Step 9: Commit**

```bash
git add crates/worker/src/routes/webhook_telegram.rs
git commit -m "refactor(webhook-telegram): localised welcome flow with transition routing"
```

---

## Task 8: Fix hardcoded language in agent prompt

**Files:**
- Modify: `crates/agent/src/loop_.rs`

- [ ] **Step 1: Locate the current hardcode**

In `crates/agent/src/loop_.rs`, find `fn build_prompt_context` (around line 237). Line ~273 currently reads:

```rust
language: "fr".to_string(),                            // Plan C : read from settings
```

The comment violates CLAUDE.md's no-plan-identifier rule. The implementation hardcodes French.

- [ ] **Step 2: Check what's available in `ToolContext`**

Read the surrounding function (lines 237-290 of `loop_.rs`) to see what fields `ctx: &ToolContext<'_>` exposes. Specifically look for:
- Anything that identifies the current workspace (slug, db, name).
- Anything per-message/per-member (e.g. a member id).

If `ctx` exposes a workspace slug and you have access to `index_db` through it, use `db::lookup_workspace_by_slug(&index_db, slug)` and read `.locale`. If `ctx` only exposes the workspace DB (not the index DB), you have two choices:
- (a) Add a `locale` field to the `ToolContext` / caller passes the locale in.
- (b) Add a settings row in the workspace DB mirroring the locale.

Prefer (a) — threading the locale from the caller is simpler and keeps workspaces_meta as the single source of truth.

- [ ] **Step 3: Thread the locale through**

In `build_prompt_context`, accept the locale as a parameter (or read it from an existing field on `ToolContext` if present). The minimum change:

```rust
language: ctx.workspace_locale.clone(),
```

(assuming `ToolContext` gains a `workspace_locale: String` field). If `ToolContext` does not yet have this field, add it to the struct definition in whatever file defines `ToolContext` (likely `crates/agent/src/loop_.rs` or a neighboring module), and update all call sites that construct `ToolContext` to pass the locale read from `WorkspaceMetaRow.locale`.

If a member-specific locale is known (e.g. `members.locale` column from migration 0009), prefer it over the workspace default — but that's out of scope for this task unless the existing code already reads per-member locale.

- [ ] **Step 4: Remove the plan-identifier comment**

Ensure the replaced line has no comment referencing "Plan C" or similar internal identifiers.

- [ ] **Step 5: Compile**

```bash
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe check --target x86_64-pc-windows-msvc -p grumps-agent -p grumps-worker
```

Expected: builds clean. Fix any callers broken by the `ToolContext` signature change.

- [ ] **Step 6: Run agent tests**

```bash
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe test --target x86_64-pc-windows-msvc -p grumps-agent
```

Expected: PASS. If the test `sample_ctx` at `crates/agent/src/prompt.rs:114` hardcodes `language: "fr".into()` — that's a test helper, leave it as-is (tests can use any literal).

- [ ] **Step 7: Commit**

```bash
git add crates/agent/src/loop_.rs # + any other files touched for ToolContext signature
git commit -m "fix(agent): read language from workspace locale"
```

---

## Task 9: WhatsApp + Discord TODO markers

**Files:**
- Modify: `crates/messaging/src/whatsapp.rs`
- Modify: `crates/messaging/src/discord.rs`

- [ ] **Step 1: Add header comment to `whatsapp.rs`**

At the top of `crates/messaging/src/whatsapp.rs`, before the `use` declarations, prepend:

```rust
// TODO(onboarding-parity): localised welcome flow with admin-promotion
// detection, workspace locale resolution, and setChatDescription-equivalent
// group metadata update. Mirror the Telegram implementation once WhatsApp
// Business API surfaces group admin events reliably.
// Reference spec: docs/superpowers/specs/2026-04-21-telegram-onboarding-ux-design.md
```

- [ ] **Step 2: Add header comment to `discord.rs`**

Same treatment for `crates/messaging/src/discord.rs`:

```rust
// TODO(onboarding-parity): localised welcome flow with workspace locale
// resolution and channel-topic update equivalent to Telegram's
// setChatDescription. Mirror the Telegram implementation when the Discord
// adapter graduates beyond MVP signature verification.
// Reference spec: docs/superpowers/specs/2026-04-21-telegram-onboarding-ux-design.md
```

- [ ] **Step 3: Compile check**

```bash
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe check --target x86_64-pc-windows-msvc -p grumps-messaging
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/messaging/src/whatsapp.rs crates/messaging/src/discord.rs
git commit -m "docs(messaging): mark WhatsApp/Discord onboarding as TODO"
```

---

## Task 10: API endpoint `PATCH /api/w/:slug/settings/locale`

**Files:**
- Modify: `crates/worker/src/routes/workspace_api.rs`
- Modify: `crates/worker/src/lib.rs`

- [ ] **Step 1: Add the handler**

In `crates/worker/src/routes/workspace_api.rs`, append a new handler at the bottom:

```rust
// ── PATCH /api/w/:slug/settings/locale ────────────────────────────────────────

pub async fn update_locale(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = auth(&req, &ctx)?;
    let ws = resolve_workspace(&ctx).await?;
    access(&claims, &ws.slug)?;
    // Admin-only (workspace admin role).
    if !claims.workspaces.iter().any(|s| s == &ws.slug) {
        return middleware::with_cors(&req, Response::error("forbidden", 403)?);
    }
    // Fine-grained admin check. If the claims carry a per-workspace role, use it;
    // otherwise fall back to index-db lookup.
    let index_db = db::get_index_db(&ctx.env)?;
    let is_admin = crate::middleware::is_workspace_admin(&index_db, &claims.user_id, &ws.slug).await
        .unwrap_or(false);
    if !is_admin {
        return middleware::with_cors(&req, Response::error("forbidden: admin required", 403)?);
    }

    #[derive(serde::Deserialize)]
    struct Body { locale: String }
    let body: Body = req.json().await.map_err(|e| Error::RustError(format!("bad body: {e}")))?;

    // Validate via the enum — any unsupported input normalises to "en", so compare
    // with `code()` and reject if it doesn't match the requested string exactly.
    let resolved = grumps_i18n::Locale::from_code(&body.locale);
    if resolved.code() != body.locale {
        return middleware::with_cors(&req, Response::error("unsupported locale", 400)?);
    }

    db::update_workspace_locale(&index_db, &ws.slug, resolved.code()).await?;

    // Side effect: re-apply the group description in the new locale (Telegram only
    // for now — WhatsApp/Discord are no-ops).
    let ws_row = db::lookup_workspace_by_slug(&index_db, &ws.slug).await?;
    if let Some(row) = ws_row {
        // Look up the platform + channel id (not on WorkspaceMetaRow today — add a
        // dedicated fetch that returns these columns).
        if let Ok(Some((platform, channel_id))) = db::lookup_platform_channel(&index_db, &row.slug).await {
            if platform == "telegram" {
                let tg = build_tg_adapter(&ctx)?;
                let desc = grumps_i18n::t(resolved, "telegram.onboarding.description", &[("slug", &row.slug)]);
                // Reuse the helper from webhook_telegram.rs — exported below.
                let _ = crate::routes::webhook_telegram::call_set_description_public(&tg, &channel_id, &desc).await;
            }
        }
    }

    middleware::with_cors(&req, Response::from_json(&serde_json::json!({
        "ok": true,
        "locale": resolved.code()
    }))?)
}

fn build_tg_adapter(ctx: &RouteContext<()>) -> Result<grumps_messaging::telegram::TelegramAdapter> {
    Ok(grumps_messaging::telegram::TelegramAdapter::new(
        ctx.env.secret("TG_BOT_TOKEN")?.to_string(),
        ctx.env.var("TG_BOT_USERNAME")?.to_string(),
        ctx.env.secret("TG_WEBHOOK_SECRET")?.to_string(),
    ))
}
```

- [ ] **Step 2: Add the two new db.rs helpers the handler needs**

In `crates/worker/src/db.rs`, add:

```rust
/// Returns `(platform, platform_channel_id)` for the workspace, or `None` if not found.
pub async fn lookup_platform_channel(index_db: &D1Database, slug: &str) -> Result<Option<(String, String)>> {
    #[derive(Deserialize)]
    struct Row { platform: String, platform_channel_id: String }
    let row = index_db.prepare("SELECT platform, platform_channel_id FROM workspaces_meta WHERE slug = ?1")
        .bind(&[slug.into()])?.first::<Row>(None).await?;
    Ok(row.map(|r| (r.platform, r.platform_channel_id)))
}
```

- [ ] **Step 3: Expose `call_set_description` publicly**

In `crates/worker/src/routes/webhook_telegram.rs`, rename `call_set_description` → `call_set_description_public` and mark it `pub(crate)` (or expose a thin wrapper). Keep the original signature:

```rust
pub(crate) async fn call_set_description_public(
    tg: &TelegramAdapter,
    chat_id: &str,
    description: &str,
) -> Result<bool> {
    // ... same body as call_set_description from Task 7 step 5
}
```

(If you prefer, keep `call_set_description` private and add a thin `pub(crate) fn` wrapper — either works.)

- [ ] **Step 4: Confirm `is_workspace_admin` exists in middleware**

Search: `grep -rn "is_workspace_admin" crates/worker/src/`. If the function doesn't exist, add it to `crates/worker/src/middleware.rs`:

```rust
pub async fn is_workspace_admin(index_db: &D1Database, user_id: &str, slug: &str) -> worker::Result<bool> {
    #[derive(serde::Deserialize)]
    struct Row { role: String }
    let row = index_db.prepare("SELECT role FROM user_workspaces WHERE user_id = ?1 AND workspace_slug = ?2")
        .bind(&[user_id.into(), slug.into()])?.first::<Row>(None).await?;
    Ok(row.map(|r| r.role == "admin").unwrap_or(false))
}
```

If an equivalent function already exists under a different name, call that one instead (update the handler accordingly).

- [ ] **Step 5: Register the route**

In `crates/worker/src/lib.rs`, find the block where other PATCH routes are registered (search for `.patch_async`) and add:

```rust
        .patch_async("/api/w/:slug/settings/locale", routes::workspace_api::update_locale)
```

- [ ] **Step 6: Compile**

```bash
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe check --target x86_64-pc-windows-msvc -p grumps-worker
```

Expected: builds clean.

- [ ] **Step 7: Manual smoke test (deferred to SPA task)**

No unit test for the route — the existing pattern in workspace_api.rs does not have HTTP-level tests. Verification happens via the SPA integration in Task 11.

- [ ] **Step 8: Commit**

```bash
git add crates/worker/src/routes/workspace_api.rs crates/worker/src/routes/webhook_telegram.rs crates/worker/src/db.rs crates/worker/src/lib.rs crates/worker/src/middleware.rs
git commit -m "feat(api): add PATCH workspace locale endpoint"
```

---

## Task 11: SPA — functional workspace locale picker

**Files:**
- Modify: `crates/spa/src/pages/settings.rs`
- Modify: `crates/spa/src/api.rs`

- [ ] **Step 1: Add the API client method**

In `crates/spa/src/api.rs`, find the section where other workspace-scoped mutations live (search for `PATCH` or `fetch_with_token`). Add:

```rust
pub async fn update_workspace_locale(slug: &str, locale: &str) -> Result<(), String> {
    let url = format!("/api/w/{}/settings/locale", slug);
    let body = serde_json::json!({ "locale": locale }).to_string();
    // Follow the existing fetch-with-JWT pattern (look at an existing PATCH call
    // in this file — there should be a todos update that matches). Replace the
    // METHOD with "PATCH" and the body with the one above.
    // Return Err on non-2xx, Ok(()) on success.
    // [Concrete code: mirror `update_todo` or whichever adjacent PATCH helper
    //  exists in this file, substituting URL, method, and body.]
}
```

Find the nearest existing PATCH helper (e.g. `update_todo` — corresponds to the `.patch_async("/api/w/:slug/todos/:id", ...)` route). Copy its implementation and adapt.

- [ ] **Step 2: Identify the current workspace locale on page load**

In `crates/spa/src/pages/settings.rs`, near the top of the component where other workspace data is loaded (probably via a `Resource` or `use_context`), obtain the current `workspaces_meta.locale` value. If the settings page currently fetches `/api/w/:slug` (via `workspace_info`), that endpoint's response may not include `locale` yet. If it doesn't, add `locale` to the response in `crates/worker/src/routes/workspace_api.rs::workspace_info` — add `"locale": ws.locale` to the JSON body. Then re-pull on the SPA side.

- [ ] **Step 3: Replace the placeholder row**

In `crates/spa/src/pages/settings.rs` around line 83, replace:

```rust
<SettingRow label_key="settings.row.language" value="English".to_string() />
```

with a functional select, modeled on the persona picker at lines 100-108 of the same file:

```rust
<div class="flex items-center justify-between py-3" style="border-bottom: 1px solid var(--ink-08);">
    <div>
        <div class="font-medium text-sm">{move || tr("settings.row.language")}</div>
    </div>
    <select
        class="border-2 border-ink rounded-sm px-3 py-1.5 text-sm bg-transparent outline-none"
        on:change=move |ev| {
            let new_locale = event_target_value(&ev);
            let slug_owned = current_slug.get_untracked();
            spawn_local(async move {
                if api::update_workspace_locale(&slug_owned, &new_locale).await.is_ok() {
                    set_workspace_locale.set(new_locale.clone());
                    // Show the "Saved." toast — use the existing toast mechanism
                    // (search the file for other toast triggers; call the same fn
                    // with key "settings.locale.updated").
                }
            });
        }
        prop:value=workspace_locale
    >
        {grumps_i18n::Locale::ALL.iter().map(|loc| {
            let code = loc.code();
            let native = loc.native_name();
            view! {
                <option value=code>{native}</option>
            }
        }).collect_view()}
    </select>
</div>
```

Adjust signal names to match the surrounding code (`current_slug`, `workspace_locale`, `set_workspace_locale` are sketched — use whatever pattern the file already uses for workspace-scoped state).

- [ ] **Step 4: Build the SPA WASM bundle**

```bash
cd crates/spa && tailwindcss -i ./input.css -o ./dist/styles.css --minify && MSYS_NO_PATHCONV=1 trunk build --release --public-url /demo/
```

Expected: build succeeds.

- [ ] **Step 5: Manual smoke test in dev**

Start the worker + SPA locally per DEPLOY.md. Open the workspace settings page. Verify:
- The dropdown shows the 14 locales.
- The initial value reflects the workspace's stored locale (en for a fresh workspace).
- Changing the dropdown triggers the PATCH and shows the "Saved." toast.
- Refreshing the page preserves the new value.

If you can, check the Telegram group description updates after the change (requires the bot being admin in a real group — otherwise the side-effect fails silently, which is expected).

- [ ] **Step 6: Commit**

```bash
git add crates/spa/src/pages/settings.rs crates/spa/src/api.rs crates/worker/src/routes/workspace_api.rs
git commit -m "feat(spa): functional workspace locale picker in settings"
```

---

## Deployment notes

After all tasks land on `main`:

1. Apply the migration to production index DB:
   ```bash
   wrangler d1 execute <index-db-name> --file=migrations/index/0002_workspace_locale.sql --remote
   ```
2. Verify `/setprivacy` is **Enabled** for `@grumps_bot` via BotFather.
3. Deploy worker: `wrangler deploy`.
4. Deploy SPA per the usual Cloudflare Pages flow.

No backfill of existing `workspaces_meta` rows is needed — the `NOT NULL DEFAULT 'en'` handles it at ALTER time.

## Self-Review Checklist

This plan covers every section of the spec:

| Spec section | Task(s) |
|---|---|
| Goal, Architecture decision | All |
| Flow states (4 transitions) | 4 (router), 7 (handlers) |
| Message content (5 keys) | 6 |
| Locale detection | 2 (`language_code`), 7 (resolution in handler) |
| i18n key layout + translator rules | 6 |
| `workspaces_meta.locale` column | 1 (migration), 5 (read/write) |
| TgUser `language_code` | 2 |
| TgChatMemberUpdated `old_chat_member` | 3 |
| Agent prompt language | 8 |
| WhatsApp/Discord TODO markers | 9 |
| DEPLOY.md privacy note | 1 |
| SPA locale picker | 11 |
| API `PATCH /api/w/:slug/settings/locale` | 10 |
| Description re-trigger on locale change | 10 |
| setChatDescription response parsing | 7 |
| Tests (router + struct parsing) | 3, 4 |

No placeholders. Types are consistent across tasks (`Transition` enum in Task 4 is used in Task 7; `call_set_description` in Task 7 is exported as `call_set_description_public` for Task 10; `update_workspace_locale` in Task 5 is called by Tasks 7 and 10; `WorkspaceMetaRow.locale` in Task 5 is read by Tasks 7, 10, 11).
