# Grumps Agent — Plan A : Foundation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Poser toute l'infrastructure de données et de scheduling pour permettre les Plans B-G : nouvelles tables D1 (memory_entries, events, scheduled_actions, agent_sessions), Durable Object `WorkspaceScheduler` avec alarmes, CRUD de base via REST, ingestion RAG (embeddings → Vectorize), et migration des workspaces existants.

**Architecture:** Extension du stack 100% Cloudflare existant. 4 nouvelles tables D1 par workspace, 1 nouveau Durable Object par workspace pour le scheduling sans polling, 1 binding Vectorize partagé pour le RAG, 1 binding Workers AI pour les embeddings. Routes REST CRUD scaffoldées (les use-cases agentiques eux-mêmes arrivent en Plan B).

**Tech Stack:** Rust + workers-rs 0.8, D1 (REST API pour workspace, native binding pour Index), Vectorize, Workers AI (`@cf/baai/bge-m3`), Durable Objects avec `state.storage().set_alarm()`. Tests : `cargo test --workspace --target x86_64-pc-windows-msvc`. Spec source : `docs/superpowers/specs/2026-04-19-grumps-agent-design.md`.

## Dependency Philosophy

**Principe** : éviter les crates **non maintenues** ou **niche** (faible adoption). Les crates massivement utilisées et activement maintenues (`serde`, `chrono`, `reqwest`, `thiserror`, `jsonwebtoken`, etc.) sont OK — pas de dogme anti-dépendances.

**Choix concrets pour Plan A** :

| Crate | Décision | Raison |
|---|---|---|
| `rrule` | ❌ Non | Non maintenu depuis ~1 an, support wasm32 incertain → implémentation manuelle des 5 cas du spec § 7.6 (~150 lignes Rust, ne touche que `chrono` déjà présent) |
| `serde_urlencoded` | ❌ Non (mais OK si on le voulait) | Pas dogmatique : juste qu'un mini-helper de ~10 lignes via `worker::Url::query_pairs()` couvre nos besoins (3-4 params/endpoint) sans rien ajouter |
| `jsonschema` | ❌ Non | `serde::Deserialize` strict sur les structs de body suffit |
| HTTP client côté worker | `worker::Fetch` (déjà inclus) | Pas de `reqwest` côté wasm — Fetch est l'API native |
| `reqwest` en dev-deps | ✅ Oui | Massivement utilisé, mature → tests d'intégration native (Task 27). Jamais bundlé en prod wasm. |

**Toutes les autres deps** (`serde`, `serde_json`, `chrono`, `uuid`, `thiserror`, `worker = "0.8"`) sont déjà au workspace, on les réutilise.

Si tu identifies une crate maintenue + populaire qui ferait gagner > 100 lignes propres dans le code à écrire, ajoute-la sans demander.

**Spec sections couvertes** : 5 (data model), 6 (mémoire — CRUD + RAG, sans auto-extract), 7 (scheduling — DOs + condition + RRULE + executor sans agent_task), 9.1-9.2 (events table + endpoints, sans aggregation/iCal), 15 (migration). Plans futurs : B (agent loop + tools), C (calendrier complet + iCal), D (web search), E (auto-extract + proactif), F (SPA pages), G (deployment runbook).

---

## File Structure

### Nouveaux crates (3 + 1 en plan B)

```
crates/
├── memory/                     ★ Plan A
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs              # types MemoryEntry, MemoryKind, MemorySource
├── scheduler/                  ★ Plan A
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs              # types + re-exports
│       ├── action.rs           # ScheduledAction, ActionType, ActionStatus
│       ├── condition.rs        # Condition enum + evaluator
│       ├── recurrence.rs       # RRULE parsing + next_occurrence
│       └── session.rs          # AgentSession (utilisé par Plan B)
└── calendar/                   ★ Plan A (partial — events only)
    ├── Cargo.toml
    └── src/
        └── lib.rs              # types Event, attendees
                                # Aggregation + iCal en Plan C
```

### Worker (modifications)

```
crates/worker/
├── Cargo.toml                  # MODIFY : add deps grumps-memory, grumps-scheduler, grumps-calendar
└── src/
    ├── lib.rs                  # MODIFY : routes memory/events/scheduled + DO export
    ├── db.rs                   # MODIFY : nouveaux WorkspaceDb methods
    ├── routes/
    │   ├── memory.rs           ★ NEW
    │   ├── events.rs           ★ NEW
    │   └── scheduled.rs        ★ NEW
    ├── durable_objects/
    │   ├── mod.rs              ★ NEW
    │   └── scheduler.rs        ★ NEW : WorkspaceScheduler DO
    ├── scheduler_executor.rs   ★ NEW : dispatch action_type → handler
    ├── rag.rs                  ★ NEW : embed + ingest + query helpers
    ├── routes/webhook.rs       # MODIFY : ingest message into RAG
    ├── routes/webhook_telegram.rs  # MODIFY : idem
    ├── routes/webhook_discord.rs   # MODIFY : idem
    └── provisioning.rs         # MODIFY : apply migrations 0002/0003/0004 to new ws
```

### Migrations

```
migrations/workspace/
├── 0001_init.sql               (existant)
├── 0002_memory.sql             ★ NEW
├── 0003_calendar.sql           ★ NEW
└── 0004_scheduling.sql         ★ NEW
```

### Configuration

```
wrangler.toml                   # MODIFY : Vectorize, Workers AI, DO bindings
Cargo.toml (workspace root)     # MODIFY : add 3 new crates as members
```

---

## Conventions et rappels avant de commencer

- **D1 pattern existant** : workspace D1 accédé via `WorkspaceDb<'a>` qui wrap `D1RestClient`. Toutes nouvelles méthodes vont sur cette struct dans `worker/src/db.rs`. L'Index DB utilise le binding natif `D1Database`.
- **Tests** : ajouter `#[cfg(test)]` modules dans chaque crate pure (memory, scheduler, calendar) pour la logique sans I/O. Tests intégrés via worker = manuels avec `wrangler dev` ou `cargo test --workspace --target x86_64-pc-windows-msvc` pour ce qui est natif.
- **TDD** : pour chaque méthode/fonction, écrire le test d'abord, le voir échouer, implémenter le minimum pour passer, refactorer si besoin.
- **Commits** : 1 commit par tâche complète (tous steps verts), pas plus gros.
- **Pas de placeholder** : tout est explicite, code complet en Rust dans chaque step.

---

## Task 1 : Update workspace Cargo.toml + scaffolding new crates

**Files:**
- Modify: `Cargo.toml` (workspace)
- Create: `crates/memory/Cargo.toml`
- Create: `crates/memory/src/lib.rs`
- Create: `crates/scheduler/Cargo.toml`
- Create: `crates/scheduler/src/lib.rs`
- Create: `crates/calendar/Cargo.toml`
- Create: `crates/calendar/src/lib.rs`

- [ ] **Step 1: Add new crates to workspace members**

Modify `Cargo.toml` à la racine, dans `[workspace] members = [...]` :

```toml
[workspace]
resolver = "2"
members = [
    "crates/core",
    "crates/nlu",
    "crates/messaging",
    "crates/worker",
    "crates/spa",
    "crates/memory",
    "crates/scheduler",
    "crates/calendar",
]

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde", "wasmbind"] }
uuid = { version = "1", features = ["v4", "js", "serde"] }
thiserror = "2"
```

- [ ] **Step 2: Create grumps-memory crate**

Create `crates/memory/Cargo.toml` :

```toml
[package]
name = "grumps-memory"
version = "0.1.0"
edition = "2021"

[dependencies]
grumps-core = { path = "../core" }
serde.workspace = true
serde_json.workspace = true
chrono.workspace = true
uuid.workspace = true
thiserror.workspace = true
```

Create `crates/memory/src/lib.rs` :

```rust
//! Memory layer for Grumps : structured workspace memory.
//! See spec § 5.1 and § 6 for schema and behavior.

pub mod types;
pub use types::*;
```

Create `crates/memory/src/types.rs` (we'll fill content in Task 5).

- [ ] **Step 3: Create grumps-scheduler crate**

Create `crates/scheduler/Cargo.toml` :

```toml
[package]
name = "grumps-scheduler"
version = "0.1.0"
edition = "2021"

[dependencies]
grumps-core = { path = "../core" }
serde.workspace = true
serde_json.workspace = true
chrono.workspace = true
uuid.workspace = true
thiserror.workspace = true
```

Create `crates/scheduler/src/lib.rs` :

```rust
//! Scheduler layer for Grumps : scheduled actions, conditions, recurrence.
//! See spec § 5.3-5.4 and § 7 for schema and behavior.

pub mod action;
pub mod condition;
pub mod recurrence;
pub mod session;

pub use action::*;
pub use condition::*;
pub use session::*;
```

Create empty `crates/scheduler/src/{action,condition,recurrence,session}.rs` (filled in tasks 12-16).

- [ ] **Step 4: Create grumps-calendar crate**

Create `crates/calendar/Cargo.toml` :

```toml
[package]
name = "grumps-calendar"
version = "0.1.0"
edition = "2021"

[dependencies]
grumps-core = { path = "../core" }
serde.workspace = true
serde_json.workspace = true
chrono.workspace = true
uuid.workspace = true
thiserror.workspace = true
```

Create `crates/calendar/src/lib.rs` :

```rust
//! Calendar layer for Grumps : events, aggregation, iCal export.
//! Plan A scope : event types only.
//! See spec § 5.2 and § 9 for schema and behavior.

pub mod event;
pub use event::*;
```

Create empty `crates/calendar/src/event.rs` (filled in Task 9).

- [ ] **Step 5: Verify workspace compiles**

Run: `cargo check --workspace --target x86_64-pc-windows-msvc`

Expected: `Compiling grumps-memory v0.1.0`, `grumps-scheduler v0.1.0`, `grumps-calendar v0.1.0` with zero errors (warnings about unused are OK).

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/memory crates/scheduler crates/calendar
git commit -m "scaffold: new crates grumps-memory, grumps-scheduler, grumps-calendar"
```

---

## Task 2 : Migration 0002 — memory_entries

**Files:**
- Create: `migrations/workspace/0002_memory.sql`

- [ ] **Step 1: Write the migration SQL**

Create `migrations/workspace/0002_memory.sql` :

```sql
-- Memory layer : structured workspace memory
-- See spec § 5.1

CREATE TABLE IF NOT EXISTS memory_entries (
    id              TEXT PRIMARY KEY,
    key             TEXT,
    value           TEXT NOT NULL,
    kind            TEXT NOT NULL,
    related_member  TEXT REFERENCES members(id),
    tags            TEXT DEFAULT '[]',
    source          TEXT NOT NULL,
    confidence      REAL DEFAULT 1.0,
    pinned          INTEGER DEFAULT 0,
    expires_at      TEXT,
    created_by      TEXT REFERENCES members(id),
    created_at      TEXT DEFAULT (datetime('now')),
    updated_at      TEXT DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_mem_kind ON memory_entries(kind);
CREATE INDEX IF NOT EXISTS idx_mem_pinned ON memory_entries(pinned) WHERE pinned = 1;
CREATE INDEX IF NOT EXISTS idx_mem_member ON memory_entries(related_member);
CREATE INDEX IF NOT EXISTS idx_mem_expires ON memory_entries(expires_at) WHERE expires_at IS NOT NULL;

CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
    key, value,
    content=memory_entries,
    content_rowid=rowid
);

CREATE TRIGGER IF NOT EXISTS memory_ai AFTER INSERT ON memory_entries BEGIN
    INSERT INTO memory_fts(rowid, key, value) VALUES (new.rowid, new.key, new.value);
END;

CREATE TRIGGER IF NOT EXISTS memory_ad AFTER DELETE ON memory_entries BEGIN
    INSERT INTO memory_fts(memory_fts, rowid, key, value)
        VALUES('delete', old.rowid, old.key, old.value);
END;

CREATE TRIGGER IF NOT EXISTS memory_au AFTER UPDATE ON memory_entries BEGIN
    INSERT INTO memory_fts(memory_fts, rowid, key, value)
        VALUES('delete', old.rowid, old.key, old.value);
    INSERT INTO memory_fts(rowid, key, value)
        VALUES (new.rowid, new.key, new.value);
END;
```

- [ ] **Step 2: Apply to local dev D1 to verify it parses**

Run: `wrangler d1 execute grumps-index --command "SELECT 1"` (just to verify wrangler works locally)

Then create a temp test DB and apply the migration to a workspace D1 (use a known dev workspace database_id) :

```bash
wrangler d1 execute <DEV_WORKSPACE_DB_ID> --file=migrations/workspace/0002_memory.sql --remote
```

Expected: `Executed N queries successfully`.

If you don't have a dev workspace yet, skip this check — the migration script in Task 28 will exercise it.

- [ ] **Step 3: Commit**

```bash
git add migrations/workspace/0002_memory.sql
git commit -m "feat(db): migration 0002 — memory_entries table + FTS"
```

---

## Task 3 : Migration 0003 — events

**Files:**
- Create: `migrations/workspace/0003_calendar.sql`

- [ ] **Step 1: Write the migration SQL**

Create `migrations/workspace/0003_calendar.sql` :

```sql
-- Calendar layer : events table
-- See spec § 5.2

CREATE TABLE IF NOT EXISTS events (
    id              TEXT PRIMARY KEY,
    title           TEXT NOT NULL,
    description     TEXT,
    starts_at       TEXT NOT NULL,
    ends_at         TEXT,
    all_day         INTEGER DEFAULT 0,
    location        TEXT,
    recurrence      TEXT,
    attendees       TEXT DEFAULT '[]',
    color           TEXT DEFAULT 'teal',
    source          TEXT DEFAULT 'web',
    related_todo_id TEXT REFERENCES todos(id),
    created_by      TEXT REFERENCES members(id),
    created_at      TEXT DEFAULT (datetime('now')),
    updated_at      TEXT DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_events_starts ON events(starts_at);
CREATE INDEX IF NOT EXISTS idx_events_recur ON events(recurrence) WHERE recurrence IS NOT NULL;
```

- [ ] **Step 2: Commit**

```bash
git add migrations/workspace/0003_calendar.sql
git commit -m "feat(db): migration 0003 — events table"
```

---

## Task 4 : Migration 0004 — scheduling + sessions + settings

**Files:**
- Create: `migrations/workspace/0004_scheduling.sql`

- [ ] **Step 1: Write the migration SQL**

Create `migrations/workspace/0004_scheduling.sql` :

```sql
-- Scheduling layer + agent sessions + new settings
-- See spec § 5.3-5.5

CREATE TABLE IF NOT EXISTS scheduled_actions (
    id              TEXT PRIMARY KEY,
    action_type     TEXT NOT NULL,
    title           TEXT NOT NULL,
    trigger_at      TEXT NOT NULL,
    recurrence      TEXT,
    condition       TEXT,
    payload         TEXT NOT NULL,
    target_chat     TEXT NOT NULL DEFAULT 'group',
    status          TEXT DEFAULT 'pending',
    last_fired_at   TEXT,
    last_error      TEXT,
    fire_count      INTEGER DEFAULT 0,
    created_by      TEXT REFERENCES members(id),
    created_at      TEXT DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_sched_fire ON scheduled_actions(trigger_at) WHERE status = 'pending';
CREATE INDEX IF NOT EXISTS idx_sched_status ON scheduled_actions(status);

CREATE TABLE IF NOT EXISTS agent_sessions (
    id              TEXT PRIMARY KEY,
    member_id       TEXT REFERENCES members(id),
    last_message_at TEXT NOT NULL,
    expires_at      TEXT NOT NULL,
    messages        TEXT NOT NULL,
    pending_action  TEXT,
    created_at      TEXT DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_sessions_member ON agent_sessions(member_id, expires_at);

-- New settings keys (insert if not present)
INSERT OR IGNORE INTO settings (key, value) VALUES ('proactive_mode', 'false');
INSERT OR IGNORE INTO settings (key, value) VALUES ('proactive_consent_at', '');
INSERT OR IGNORE INTO settings (key, value) VALUES ('proactive_max_per_hour', '3');
INSERT OR IGNORE INTO settings (key, value) VALUES ('auto_memory', 'false');
INSERT OR IGNORE INTO settings (key, value) VALUES ('auto_memory_consent_at', '');
INSERT OR IGNORE INTO settings (key, value) VALUES ('agent_quota_used_month', '0');
INSERT OR IGNORE INTO settings (key, value) VALUES ('web_search_quota_used_month', '0');
INSERT OR IGNORE INTO settings (key, value) VALUES ('agent_persona', 'default');
INSERT OR IGNORE INTO settings (key, value) VALUES ('ical_token', '');
```

- [ ] **Step 2: Commit**

```bash
git add migrations/workspace/0004_scheduling.sql
git commit -m "feat(db): migration 0004 — scheduled_actions + agent_sessions + settings"
```

---

## Task 5 : Memory types (grumps-memory)

**Files:**
- Modify: `crates/memory/src/types.rs`
- Test: `crates/memory/src/types.rs` (inline `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing test for serialization**

Modify `crates/memory/src/types.rs` :

```rust
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    Fact,
    Person,
    Decision,
    Preference,
    Place,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MemorySource {
    ChatExplicit,
    ChatAuto,
    Web,
    Agent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub key: Option<String>,
    pub value: String,
    pub kind: MemoryKind,
    pub related_member: Option<String>,
    pub tags: Vec<String>,
    pub source: MemorySource,
    pub confidence: f64,
    pub pinned: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NewMemoryEntry {
    pub key: Option<String>,
    pub value: String,
    pub kind: MemoryKind,
    pub related_member: Option<String>,
    pub tags: Vec<String>,
    pub source: MemorySource,
    pub confidence: Option<f64>,
    pub pinned: Option<bool>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_by: Option<String>,
}

impl Default for MemoryKind {
    fn default() -> Self { MemoryKind::Other }
}

impl Default for MemorySource {
    fn default() -> Self { MemorySource::Web }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_serializes_snake_case() {
        let json = serde_json::to_string(&MemoryKind::Fact).unwrap();
        assert_eq!(json, r#""fact""#);
        let json = serde_json::to_string(&MemoryKind::Other).unwrap();
        assert_eq!(json, r#""other""#);
    }

    #[test]
    fn source_serializes_kebab_case() {
        let json = serde_json::to_string(&MemorySource::ChatExplicit).unwrap();
        assert_eq!(json, r#""chat-explicit""#);
        let json = serde_json::to_string(&MemorySource::ChatAuto).unwrap();
        assert_eq!(json, r#""chat-auto""#);
    }

    #[test]
    fn entry_roundtrips_json() {
        let now = Utc::now();
        let e = MemoryEntry {
            id: "m1".into(), key: Some("wifi".into()), value: "abc".into(),
            kind: MemoryKind::Fact, related_member: None, tags: vec!["home".into()],
            source: MemorySource::ChatExplicit, confidence: 1.0, pinned: true,
            expires_at: None, created_by: Some("u1".into()),
            created_at: now, updated_at: now,
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: MemoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.value, "abc");
        assert!(back.pinned);
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test -p grumps-memory --target x86_64-pc-windows-msvc`

Expected: `test result: ok. 3 passed; 0 failed`.

- [ ] **Step 3: Commit**

```bash
git add crates/memory/src/types.rs
git commit -m "feat(memory): MemoryEntry, MemoryKind, MemorySource types with serde"
```

---

## Task 6 : Memory CRUD on WorkspaceDb (worker)

**Files:**
- Modify: `crates/worker/Cargo.toml`
- Modify: `crates/worker/src/db.rs`
- Test: integration via Task 22 (REST routes)

- [ ] **Step 1: Add grumps-memory dep to worker**

Modify `crates/worker/Cargo.toml` `[dependencies]` :

```toml
grumps-memory = { path = "../memory" }
```

- [ ] **Step 2: Add CRUD methods to WorkspaceDb**

Append to `crates/worker/src/db.rs` (after existing impl block of `WorkspaceDb`) :

```rust
// =============================================
// Memory CRUD (see spec § 6.1)
// =============================================

use grumps_memory::{MemoryEntry, MemoryKind, MemorySource, NewMemoryEntry};

#[derive(Deserialize, Debug, Clone)]
pub struct MemoryRow {
    pub id: String,
    pub key: Option<String>,
    pub value: String,
    pub kind: String,
    pub related_member: Option<String>,
    pub tags: String,             // JSON array as TEXT
    pub source: String,
    pub confidence: f64,
    pub pinned: i64,
    pub expires_at: Option<String>,
    pub created_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl<'a> WorkspaceDb<'a> {
    pub async fn create_memory(&self, entry: &NewMemoryEntry) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let kind = serde_json::to_value(&entry.kind).unwrap()
            .as_str().unwrap_or("other").to_string();
        let source = serde_json::to_value(&entry.source).unwrap()
            .as_str().unwrap_or("web").to_string();
        let tags_json = serde_json::to_string(&entry.tags).unwrap_or_else(|_| "[]".into());
        let pinned = if entry.pinned.unwrap_or(false) { 1 } else { 0 };
        let confidence = entry.confidence.unwrap_or(1.0);
        let expires_at_json: serde_json::Value = entry.expires_at
            .map(|d| serde_json::Value::String(d.to_rfc3339()))
            .unwrap_or(serde_json::Value::Null);

        self.q(
            "INSERT INTO memory_entries (id, key, value, kind, related_member, tags, source, confidence, pinned, expires_at, created_by) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            vec![
                id.clone().into(),
                entry.key.clone().map(serde_json::Value::String).unwrap_or(serde_json::Value::Null),
                entry.value.clone().into(),
                kind.into(),
                entry.related_member.clone().map(serde_json::Value::String).unwrap_or(serde_json::Value::Null),
                tags_json.into(),
                source.into(),
                confidence.into(),
                pinned.into(),
                expires_at_json,
                entry.created_by.clone().map(serde_json::Value::String).unwrap_or(serde_json::Value::Null),
            ],
        ).await?;
        Ok(id)
    }

    pub async fn get_memory(&self, id: &str) -> Result<Option<MemoryEntry>> {
        let resp = self.q(
            "SELECT id, key, value, kind, related_member, tags, source, confidence, pinned, expires_at, created_by, created_at, updated_at \
             FROM memory_entries WHERE id = ?1",
            vec![id.into()],
        ).await?;
        let row: Option<MemoryRow> = extract_first(&resp)?;
        Ok(row.map(memory_row_to_entry))
    }

    pub async fn list_memory(&self, kind_filter: Option<&str>, source_filter: Option<&str>, limit: i64, offset: i64) -> Result<Vec<MemoryEntry>> {
        let mut sql = String::from(
            "SELECT id, key, value, kind, related_member, tags, source, confidence, pinned, expires_at, created_by, created_at, updated_at \
             FROM memory_entries WHERE (expires_at IS NULL OR expires_at > datetime('now'))"
        );
        let mut params: Vec<serde_json::Value> = vec![];
        if let Some(k) = kind_filter {
            params.push(k.into());
            sql.push_str(&format!(" AND kind = ?{}", params.len()));
        }
        if let Some(s) = source_filter {
            params.push(s.into());
            sql.push_str(&format!(" AND source = ?{}", params.len()));
        }
        sql.push_str(" ORDER BY pinned DESC, updated_at DESC");
        params.push(limit.into());
        sql.push_str(&format!(" LIMIT ?{}", params.len()));
        params.push(offset.into());
        sql.push_str(&format!(" OFFSET ?{}", params.len()));

        let resp = self.q(&sql, params).await?;
        let rows: Vec<MemoryRow> = extract_rows(&resp)?;
        Ok(rows.into_iter().map(memory_row_to_entry).collect())
    }

    pub async fn list_pinned_memory(&self) -> Result<Vec<MemoryEntry>> {
        let resp = self.q(
            "SELECT id, key, value, kind, related_member, tags, source, confidence, pinned, expires_at, created_by, created_at, updated_at \
             FROM memory_entries WHERE pinned = 1 AND (expires_at IS NULL OR expires_at > datetime('now')) \
             ORDER BY updated_at DESC",
            vec![],
        ).await?;
        let rows: Vec<MemoryRow> = extract_rows(&resp)?;
        Ok(rows.into_iter().map(memory_row_to_entry).collect())
    }

    pub async fn update_memory(&self, id: &str, value: Option<&str>, pinned: Option<bool>, expires_at: Option<&str>) -> Result<bool> {
        // Coalesce-style update : only set non-None fields
        let mut sets = vec!["updated_at = datetime('now')".to_string()];
        let mut params: Vec<serde_json::Value> = vec![];
        if let Some(v) = value {
            params.push(v.into());
            sets.push(format!("value = ?{}", params.len()));
        }
        if let Some(p) = pinned {
            params.push((if p { 1 } else { 0 }).into());
            sets.push(format!("pinned = ?{}", params.len()));
        }
        if let Some(e) = expires_at {
            params.push(e.into());
            sets.push(format!("expires_at = ?{}", params.len()));
        }
        if sets.len() == 1 { return Ok(false); }
        params.push(id.into());
        let sql = format!("UPDATE memory_entries SET {} WHERE id = ?{}", sets.join(", "), params.len());
        let resp = self.q(&sql, params).await?;
        Ok(resp.meta.as_ref().and_then(|m| m.changes).unwrap_or(0) > 0)
    }

    pub async fn delete_memory(&self, id: &str) -> Result<bool> {
        let resp = self.q("DELETE FROM memory_entries WHERE id = ?1", vec![id.into()]).await?;
        Ok(resp.meta.as_ref().and_then(|m| m.changes).unwrap_or(0) > 0)
    }

    pub async fn search_memory_fts(&self, query: &str, limit: i64) -> Result<Vec<MemoryEntry>> {
        // FTS5 query on key + value
        let resp = self.q(
            "SELECT m.id, m.key, m.value, m.kind, m.related_member, m.tags, m.source, m.confidence, m.pinned, m.expires_at, m.created_by, m.created_at, m.updated_at \
             FROM memory_entries m JOIN memory_fts f ON m.rowid = f.rowid \
             WHERE memory_fts MATCH ?1 AND (m.expires_at IS NULL OR m.expires_at > datetime('now')) \
             ORDER BY rank LIMIT ?2",
            vec![query.into(), limit.into()],
        ).await?;
        let rows: Vec<MemoryRow> = extract_rows(&resp)?;
        Ok(rows.into_iter().map(memory_row_to_entry).collect())
    }

    pub async fn count_memory(&self) -> Result<i64> {
        #[derive(Deserialize)]
        struct Row { cnt: i64 }
        let resp = self.q("SELECT COUNT(*) as cnt FROM memory_entries", vec![]).await?;
        let row: Option<Row> = extract_first(&resp)?;
        Ok(row.map(|r| r.cnt).unwrap_or(0))
    }
}

fn memory_row_to_entry(r: MemoryRow) -> MemoryEntry {
    use chrono::{DateTime, Utc, TimeZone};
    let parse_dt = |s: &str| -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc.timestamp_opt(0, 0).unwrap())
    };
    MemoryEntry {
        id: r.id,
        key: r.key,
        value: r.value,
        kind: serde_json::from_value(serde_json::Value::String(r.kind.clone()))
            .unwrap_or(MemoryKind::Other),
        related_member: r.related_member,
        tags: serde_json::from_str(&r.tags).unwrap_or_default(),
        source: serde_json::from_value(serde_json::Value::String(r.source.clone()))
            .unwrap_or(MemorySource::Web),
        confidence: r.confidence,
        pinned: r.pinned != 0,
        expires_at: r.expires_at.as_deref().map(parse_dt),
        created_by: r.created_by,
        created_at: parse_dt(&r.created_at),
        updated_at: parse_dt(&r.updated_at),
    }
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p grumps-worker --target x86_64-pc-windows-msvc`

Expected: zero errors.

- [ ] **Step 4: Commit**

```bash
git add crates/worker/Cargo.toml crates/worker/src/db.rs
git commit -m "feat(memory): WorkspaceDb CRUD methods + FTS search"
```

---

## Task 7 : Event types (grumps-calendar)

**Files:**
- Modify: `crates/calendar/src/event.rs`

- [ ] **Step 1: Write types and serialization tests**

Modify `crates/calendar/src/event.rs` :

```rust
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventSource {
    Chat,
    Web,
    Agent,
}

impl Default for EventSource {
    fn default() -> Self { EventSource::Web }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub all_day: bool,
    pub location: Option<String>,
    pub recurrence: Option<String>,    // RRULE
    pub attendees: Vec<String>,        // member.id
    pub color: String,
    pub source: EventSource,
    pub related_todo_id: Option<String>,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NewEvent {
    pub title: String,
    pub description: Option<String>,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub all_day: bool,
    pub location: Option<String>,
    pub recurrence: Option<String>,
    pub attendees: Vec<String>,
    pub color: Option<String>,
    pub source: EventSource,
    pub related_todo_id: Option<String>,
    pub created_by: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_source_serializes_snake() {
        assert_eq!(serde_json::to_string(&EventSource::Web).unwrap(), r#""web""#);
        assert_eq!(serde_json::to_string(&EventSource::Chat).unwrap(), r#""chat""#);
    }

    #[test]
    fn event_default_color_teal() {
        // Default applies in DB layer via SQL DEFAULT, not type Default — but verify the field exists.
        let e = NewEvent {
            title: "test".into(),
            starts_at: Utc::now(),
            color: Some("teal".into()),
            ..Default::default()
        };
        assert_eq!(e.color.as_deref(), Some("teal"));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p grumps-calendar --target x86_64-pc-windows-msvc`

Expected: `test result: ok. 2 passed`.

- [ ] **Step 3: Commit**

```bash
git add crates/calendar/src/event.rs
git commit -m "feat(calendar): Event and NewEvent types with serde"
```

---

## Task 8 : Event CRUD on WorkspaceDb

**Files:**
- Modify: `crates/worker/Cargo.toml`
- Modify: `crates/worker/src/db.rs`

- [ ] **Step 1: Add grumps-calendar dep to worker**

Append to `crates/worker/Cargo.toml` `[dependencies]` :

```toml
grumps-calendar = { path = "../calendar" }
```

- [ ] **Step 2: Add Event CRUD methods to WorkspaceDb**

Append to `crates/worker/src/db.rs` :

```rust
// =============================================
// Events CRUD (see spec § 5.2 + § 9.2)
// =============================================

use grumps_calendar::{Event, EventSource, NewEvent};

#[derive(Deserialize, Debug, Clone)]
pub struct EventRow {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub starts_at: String,
    pub ends_at: Option<String>,
    pub all_day: i64,
    pub location: Option<String>,
    pub recurrence: Option<String>,
    pub attendees: String,
    pub color: String,
    pub source: String,
    pub related_todo_id: Option<String>,
    pub created_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl<'a> WorkspaceDb<'a> {
    pub async fn create_event(&self, e: &NewEvent) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let attendees_json = serde_json::to_string(&e.attendees).unwrap_or_else(|_| "[]".into());
        let source = serde_json::to_value(&e.source).unwrap()
            .as_str().unwrap_or("web").to_string();
        let color = e.color.clone().unwrap_or_else(|| "teal".into());
        let ends_at: serde_json::Value = e.ends_at
            .map(|d| serde_json::Value::String(d.to_rfc3339()))
            .unwrap_or(serde_json::Value::Null);

        self.q(
            "INSERT INTO events (id, title, description, starts_at, ends_at, all_day, location, recurrence, attendees, color, source, related_todo_id, created_by) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            vec![
                id.clone().into(),
                e.title.clone().into(),
                e.description.clone().map(serde_json::Value::String).unwrap_or(serde_json::Value::Null),
                e.starts_at.to_rfc3339().into(),
                ends_at,
                (if e.all_day { 1 } else { 0 }).into(),
                e.location.clone().map(serde_json::Value::String).unwrap_or(serde_json::Value::Null),
                e.recurrence.clone().map(serde_json::Value::String).unwrap_or(serde_json::Value::Null),
                attendees_json.into(),
                color.into(),
                source.into(),
                e.related_todo_id.clone().map(serde_json::Value::String).unwrap_or(serde_json::Value::Null),
                e.created_by.clone().map(serde_json::Value::String).unwrap_or(serde_json::Value::Null),
            ],
        ).await?;
        Ok(id)
    }

    pub async fn get_event(&self, id: &str) -> Result<Option<Event>> {
        let resp = self.q(
            "SELECT id, title, description, starts_at, ends_at, all_day, location, recurrence, attendees, color, source, related_todo_id, created_by, created_at, updated_at \
             FROM events WHERE id = ?1",
            vec![id.into()],
        ).await?;
        let row: Option<EventRow> = extract_first(&resp)?;
        Ok(row.map(event_row_to_event))
    }

    pub async fn list_events_in_range(&self, from: &str, to: &str) -> Result<Vec<Event>> {
        let resp = self.q(
            "SELECT id, title, description, starts_at, ends_at, all_day, location, recurrence, attendees, color, source, related_todo_id, created_by, created_at, updated_at \
             FROM events WHERE starts_at >= ?1 AND starts_at <= ?2 ORDER BY starts_at ASC",
            vec![from.into(), to.into()],
        ).await?;
        let rows: Vec<EventRow> = extract_rows(&resp)?;
        Ok(rows.into_iter().map(event_row_to_event).collect())
    }

    pub async fn update_event(&self, id: &str, title: Option<&str>, starts_at: Option<&str>, ends_at: Option<&str>, location: Option<&str>) -> Result<bool> {
        let mut sets = vec!["updated_at = datetime('now')".to_string()];
        let mut params: Vec<serde_json::Value> = vec![];
        if let Some(v) = title { params.push(v.into()); sets.push(format!("title = ?{}", params.len())); }
        if let Some(v) = starts_at { params.push(v.into()); sets.push(format!("starts_at = ?{}", params.len())); }
        if let Some(v) = ends_at { params.push(v.into()); sets.push(format!("ends_at = ?{}", params.len())); }
        if let Some(v) = location { params.push(v.into()); sets.push(format!("location = ?{}", params.len())); }
        if sets.len() == 1 { return Ok(false); }
        params.push(id.into());
        let sql = format!("UPDATE events SET {} WHERE id = ?{}", sets.join(", "), params.len());
        let resp = self.q(&sql, params).await?;
        Ok(resp.meta.as_ref().and_then(|m| m.changes).unwrap_or(0) > 0)
    }

    pub async fn delete_event(&self, id: &str) -> Result<bool> {
        let resp = self.q("DELETE FROM events WHERE id = ?1", vec![id.into()]).await?;
        Ok(resp.meta.as_ref().and_then(|m| m.changes).unwrap_or(0) > 0)
    }
}

fn event_row_to_event(r: EventRow) -> Event {
    use chrono::{DateTime, Utc, TimeZone};
    let parse_dt = |s: &str| -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc.timestamp_opt(0, 0).unwrap())
    };
    Event {
        id: r.id,
        title: r.title,
        description: r.description,
        starts_at: parse_dt(&r.starts_at),
        ends_at: r.ends_at.as_deref().map(parse_dt),
        all_day: r.all_day != 0,
        location: r.location,
        recurrence: r.recurrence,
        attendees: serde_json::from_str(&r.attendees).unwrap_or_default(),
        color: r.color,
        source: serde_json::from_value(serde_json::Value::String(r.source.clone()))
            .unwrap_or(EventSource::Web),
        related_todo_id: r.related_todo_id,
        created_by: r.created_by,
        created_at: parse_dt(&r.created_at),
        updated_at: parse_dt(&r.updated_at),
    }
}
```

- [ ] **Step 3: Verify compiles**

Run: `cargo check -p grumps-worker --target x86_64-pc-windows-msvc`

Expected: zero errors.

- [ ] **Step 4: Commit**

```bash
git add crates/worker/Cargo.toml crates/worker/src/db.rs
git commit -m "feat(calendar): WorkspaceDb Event CRUD methods"
```

---

## Task 9 : Scheduled action types (grumps-scheduler)

**Files:**
- Modify: `crates/scheduler/src/action.rs`

- [ ] **Step 1: Write types and tests**

Modify `crates/scheduler/src/action.rs` :

```rust
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    Reminder,
    FollowUp,
    Recap,
    AgentTask,
    EventNotify,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionStatus {
    Pending,
    Firing,
    Done,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledAction {
    pub id: String,
    pub action_type: ActionType,
    pub title: String,
    pub trigger_at: DateTime<Utc>,
    pub recurrence: Option<String>,    // RRULE
    pub condition: Option<serde_json::Value>,
    pub payload: serde_json::Value,
    pub target_chat: String,           // "group" only at launch
    pub status: ActionStatus,
    pub last_fired_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub fire_count: i64,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewScheduledAction {
    pub action_type: ActionType,
    pub title: String,
    pub trigger_at: DateTime<Utc>,
    pub recurrence: Option<String>,
    pub condition: Option<serde_json::Value>,
    pub payload: serde_json::Value,
    pub target_chat: Option<String>,
    pub created_by: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_type_snake() {
        assert_eq!(serde_json::to_string(&ActionType::AgentTask).unwrap(), r#""agent_task""#);
        assert_eq!(serde_json::to_string(&ActionType::EventNotify).unwrap(), r#""event_notify""#);
    }

    #[test]
    fn status_snake() {
        assert_eq!(serde_json::to_string(&ActionStatus::Pending).unwrap(), r#""pending""#);
        assert_eq!(serde_json::to_string(&ActionStatus::Firing).unwrap(), r#""firing""#);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p grumps-scheduler --target x86_64-pc-windows-msvc`

Expected: `test result: ok. 2 passed`.

- [ ] **Step 3: Commit**

```bash
git add crates/scheduler/src/action.rs
git commit -m "feat(scheduler): ScheduledAction types"
```

---

## Task 10 : Condition evaluator (5 types)

**Files:**
- Modify: `crates/scheduler/src/condition.rs`

- [ ] **Step 1: Write the failing tests**

Modify `crates/scheduler/src/condition.rs` :

```rust
//! Condition evaluator for scheduled actions B (suivis conditionnels).
//! See spec § 7.5 — 5 types prédéfinis.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Duration};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Condition {
    NoMessageMatching { since: DateTime<Utc>, match_keywords: Vec<String>, min_message_count: u32 },
    MemberActiveAfter { member_id: String, after: DateTime<Utc> },
    TodoStatus { todo_id: String, #[serde(default)] status_not: Option<String>, #[serde(default)] status_is: Option<String> },
    MemberInactiveFor { member_id: String, duration_seconds: i64 },
    KeywordAppeared { keywords: Vec<String>, since: DateTime<Utc> },
}

/// Context provided by the worker when evaluating a condition.
/// Plan A : trait définie ; les méthodes sont stubbed/mockées dans le test ici.
/// L'implémentation worker (DB-backed) arrive en Task 13.
pub trait ConditionContext {
    fn count_messages_matching(&self, since: DateTime<Utc>, keywords: &[String]) -> i64;
    fn last_active_at(&self, member_id: &str) -> Option<DateTime<Utc>>;
    fn todo_status_now(&self, todo_id: &str) -> Option<String>;
    fn now(&self) -> DateTime<Utc>;
}

pub fn evaluate<C: ConditionContext>(cond: &Condition, ctx: &C) -> bool {
    match cond {
        Condition::NoMessageMatching { since, match_keywords, min_message_count } => {
            ctx.count_messages_matching(*since, match_keywords) < *min_message_count as i64
        }
        Condition::MemberActiveAfter { member_id, after } => {
            ctx.last_active_at(member_id).map(|t| t > *after).unwrap_or(false)
        }
        Condition::TodoStatus { todo_id, status_not, status_is } => {
            let status = ctx.todo_status_now(todo_id);
            if let Some(s_not) = status_not {
                if status.as_deref() != Some(s_not.as_str()) { return true; }
            }
            if let Some(s_is) = status_is {
                if status.as_deref() == Some(s_is.as_str()) { return true; }
            }
            // If neither matched, condition is false
            status_not.is_none() && status_is.is_none()
        }
        Condition::MemberInactiveFor { member_id, duration_seconds } => {
            let now = ctx.now();
            match ctx.last_active_at(member_id) {
                None => true,                                            // never active = inactive
                Some(t) => now.signed_duration_since(t) >= Duration::seconds(*duration_seconds),
            }
        }
        Condition::KeywordAppeared { keywords, since } => {
            ctx.count_messages_matching(*since, keywords) > 0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockCtx {
        msg_count: i64,
        last_active: Option<DateTime<Utc>>,
        todo_status: Option<String>,
        now: DateTime<Utc>,
    }
    impl ConditionContext for MockCtx {
        fn count_messages_matching(&self, _: DateTime<Utc>, _: &[String]) -> i64 { self.msg_count }
        fn last_active_at(&self, _: &str) -> Option<DateTime<Utc>> { self.last_active }
        fn todo_status_now(&self, _: &str) -> Option<String> { self.todo_status.clone() }
        fn now(&self) -> DateTime<Utc> { self.now }
    }

    fn ctx() -> MockCtx {
        MockCtx { msg_count: 0, last_active: None, todo_status: None, now: Utc::now() }
    }

    #[test]
    fn no_message_matching_fires_when_none() {
        let cond = Condition::NoMessageMatching {
            since: Utc::now(), match_keywords: vec!["x".into()], min_message_count: 1,
        };
        assert!(evaluate(&cond, &ctx()));
    }

    #[test]
    fn no_message_matching_skips_when_present() {
        let cond = Condition::NoMessageMatching {
            since: Utc::now(), match_keywords: vec!["x".into()], min_message_count: 1,
        };
        let c = MockCtx { msg_count: 1, ..ctx() };
        assert!(!evaluate(&cond, &c));
    }

    #[test]
    fn member_active_after_fires_when_recent() {
        let cond = Condition::MemberActiveAfter {
            member_id: "m1".into(),
            after: Utc::now() - Duration::days(1),
        };
        let c = MockCtx { last_active: Some(Utc::now()), ..ctx() };
        assert!(evaluate(&cond, &c));
    }

    #[test]
    fn todo_status_not_done_fires_when_status_open() {
        let cond = Condition::TodoStatus {
            todo_id: "t1".into(), status_not: Some("done".into()), status_is: None,
        };
        let c = MockCtx { todo_status: Some("open".into()), ..ctx() };
        assert!(evaluate(&cond, &c));
    }

    #[test]
    fn todo_status_not_done_skips_when_done() {
        let cond = Condition::TodoStatus {
            todo_id: "t1".into(), status_not: Some("done".into()), status_is: None,
        };
        let c = MockCtx { todo_status: Some("done".into()), ..ctx() };
        assert!(!evaluate(&cond, &c));
    }

    #[test]
    fn member_inactive_for_fires_when_long_silent() {
        let cond = Condition::MemberInactiveFor {
            member_id: "m1".into(), duration_seconds: 3600,
        };
        let c = MockCtx {
            now: Utc::now(),
            last_active: Some(Utc::now() - Duration::hours(2)),
            ..ctx()
        };
        assert!(evaluate(&cond, &c));
    }

    #[test]
    fn member_inactive_skips_when_recent() {
        let cond = Condition::MemberInactiveFor {
            member_id: "m1".into(), duration_seconds: 3600,
        };
        let c = MockCtx {
            now: Utc::now(),
            last_active: Some(Utc::now() - Duration::seconds(60)),
            ..ctx()
        };
        assert!(!evaluate(&cond, &c));
    }

    #[test]
    fn keyword_appeared_fires_when_present() {
        let cond = Condition::KeywordAppeared {
            keywords: vec!["restau".into()], since: Utc::now() - Duration::days(1),
        };
        let c = MockCtx { msg_count: 1, ..ctx() };
        assert!(evaluate(&cond, &c));
    }

    #[test]
    fn keyword_appeared_skips_when_absent() {
        let cond = Condition::KeywordAppeared {
            keywords: vec!["restau".into()], since: Utc::now() - Duration::days(1),
        };
        assert!(!evaluate(&cond, &ctx()));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p grumps-scheduler --target x86_64-pc-windows-msvc condition`

Expected: `test result: ok. 9 passed`.

- [ ] **Step 3: Commit**

```bash
git add crates/scheduler/src/condition.rs
git commit -m "feat(scheduler): Condition evaluator with 5 types + 9 unit tests"
```

---

## Task 11 : Recurrence (RRULE) — minimal implementation

**Files:**
- Modify: `crates/scheduler/src/recurrence.rs`  *(pas de modif de Cargo.toml — aucune dep ajoutée)*

> **Decision** : ❌ on **n'utilise pas** la crate `rrule` :
> - non maintenue depuis ~1 an (dernier release courant 2024)
> - traîne `regex` + `lazy_static` + d'autres deps lourdes
> - support wasm32 incertain
>
> ✅ On implémente manuellement les 5 cas usuels listés dans le spec § 7.6 (DAILY, WEEKLY+BYDAY[+INTERVAL], MONTHLY+BYMONTHDAY, YEARLY+BYMONTH+BYMONTHDAY). ~150 lignes Rust, ne touche que `chrono` qu'on a déjà. Si on a besoin de plus tard de cas exotiques (BYSETPOS, EXDATE, etc.), on étendra ce module — la grande majorité des users n'en a pas besoin.

- [ ] **Step 1: Write failing tests for the 5 supported cases**

Modify `crates/scheduler/src/recurrence.rs` :

```rust
//! Minimal RRULE expander supporting the 5 launch-time cases :
//! - FREQ=DAILY
//! - FREQ=WEEKLY;BYDAY=...
//! - FREQ=WEEKLY;BYDAY=...;INTERVAL=N
//! - FREQ=MONTHLY;BYMONTHDAY=N
//! - FREQ=YEARLY;BYMONTH=M;BYMONTHDAY=N
//!
//! See spec § 7.6.

use chrono::{DateTime, Utc, NaiveDate, Datelike, Weekday, TimeZone, Duration, Timelike};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RruleError {
    #[error("Unsupported FREQ: {0}")]
    UnsupportedFreq(String),
    #[error("Invalid RRULE syntax: {0}")]
    InvalidSyntax(String),
    #[error("Missing required field: {0}")]
    MissingField(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rrule {
    pub freq: Freq,
    pub interval: u32,                      // default 1
    pub by_day: Vec<Weekday>,               // for WEEKLY
    pub by_month_day: Option<u32>,          // for MONTHLY/YEARLY
    pub by_month: Option<u32>,              // for YEARLY
    pub by_hour: Option<u32>,               // optional for any FREQ
    pub by_minute: Option<u32>,             // optional for any FREQ
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Freq {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

pub fn parse(rrule: &str) -> Result<Rrule, RruleError> {
    let mut freq: Option<Freq> = None;
    let mut interval = 1u32;
    let mut by_day = vec![];
    let mut by_month_day = None;
    let mut by_month = None;
    let mut by_hour = None;
    let mut by_minute = None;

    for part in rrule.split(';') {
        let mut kv = part.splitn(2, '=');
        let k = kv.next().ok_or_else(|| RruleError::InvalidSyntax(rrule.into()))?;
        let v = kv.next().ok_or_else(|| RruleError::InvalidSyntax(rrule.into()))?;
        match k {
            "FREQ" => freq = Some(match v {
                "DAILY" => Freq::Daily,
                "WEEKLY" => Freq::Weekly,
                "MONTHLY" => Freq::Monthly,
                "YEARLY" => Freq::Yearly,
                other => return Err(RruleError::UnsupportedFreq(other.into())),
            }),
            "INTERVAL" => interval = v.parse().map_err(|_| RruleError::InvalidSyntax(v.into()))?,
            "BYDAY" => by_day = v.split(',').map(parse_weekday).collect::<Result<Vec<_>, _>>()?,
            "BYMONTHDAY" => by_month_day = Some(v.parse().map_err(|_| RruleError::InvalidSyntax(v.into()))?),
            "BYMONTH" => by_month = Some(v.parse().map_err(|_| RruleError::InvalidSyntax(v.into()))?),
            "BYHOUR" => by_hour = Some(v.parse().map_err(|_| RruleError::InvalidSyntax(v.into()))?),
            "BYMINUTE" => by_minute = Some(v.parse().map_err(|_| RruleError::InvalidSyntax(v.into()))?),
            _ => { /* ignore unknown */ }
        }
    }

    Ok(Rrule {
        freq: freq.ok_or_else(|| RruleError::MissingField("FREQ".into()))?,
        interval, by_day, by_month_day, by_month, by_hour, by_minute,
    })
}

fn parse_weekday(s: &str) -> Result<Weekday, RruleError> {
    match s {
        "MO" => Ok(Weekday::Mon), "TU" => Ok(Weekday::Tue),
        "WE" => Ok(Weekday::Wed), "TH" => Ok(Weekday::Thu),
        "FR" => Ok(Weekday::Fri), "SA" => Ok(Weekday::Sat),
        "SU" => Ok(Weekday::Sun),
        _ => Err(RruleError::InvalidSyntax(s.into())),
    }
}

/// Compute the next occurrence after `from` (exclusive).
pub fn next_occurrence(rule: &Rrule, from: DateTime<Utc>) -> Option<DateTime<Utc>> {
    // Strategy : starting at `from + 1 minute`, walk day by day for up to
    // a finite limit, find the first datetime matching the rule.
    let limit_days = match rule.freq {
        Freq::Daily => 31 * rule.interval as i64,
        Freq::Weekly => 7 * rule.interval as i64 * 4,    // up to ~4 cycles
        Freq::Monthly => 366,
        Freq::Yearly => 366 * 4,
    };
    let mut candidate = from + Duration::minutes(1);
    // Snap time to BYHOUR/BYMINUTE if specified
    if let Some(h) = rule.by_hour {
        candidate = candidate.with_hour(h)?.with_minute(rule.by_minute.unwrap_or(0))?
            .with_second(0)?.with_nanosecond(0)?;
        if candidate <= from { candidate = candidate + Duration::days(1); }
    }
    for _ in 0..limit_days {
        if matches_rule(rule, candidate, from) {
            return Some(candidate);
        }
        candidate = candidate + Duration::days(1);
        if let Some(h) = rule.by_hour {
            candidate = candidate.with_hour(h)?.with_minute(rule.by_minute.unwrap_or(0))?
                .with_second(0)?.with_nanosecond(0)?;
        }
    }
    None
}

fn matches_rule(rule: &Rrule, dt: DateTime<Utc>, base: DateTime<Utc>) -> bool {
    match rule.freq {
        Freq::Daily => {
            let days_since_base = (dt.date_naive() - base.date_naive()).num_days();
            days_since_base > 0 && (days_since_base as u32) % rule.interval == 0
        }
        Freq::Weekly => {
            if rule.by_day.is_empty() { return false; }
            if !rule.by_day.contains(&dt.weekday()) { return false; }
            // INTERVAL applies to weeks
            let weeks_since_base = ((dt.date_naive() - base.date_naive()).num_days() / 7) as u32;
            weeks_since_base % rule.interval == 0
        }
        Freq::Monthly => {
            if let Some(mday) = rule.by_month_day {
                dt.day() == mday
            } else { false }
        }
        Freq::Yearly => {
            if let (Some(m), Some(mday)) = (rule.by_month, rule.by_month_day) {
                dt.month() == m && dt.day() == mday
            } else { false }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, min, 0).unwrap()
    }

    #[test]
    fn parse_daily() {
        let r = parse("FREQ=DAILY").unwrap();
        assert_eq!(r.freq, Freq::Daily);
        assert_eq!(r.interval, 1);
    }

    #[test]
    fn parse_weekly_monday() {
        let r = parse("FREQ=WEEKLY;BYDAY=MO").unwrap();
        assert_eq!(r.freq, Freq::Weekly);
        assert_eq!(r.by_day, vec![Weekday::Mon]);
    }

    #[test]
    fn parse_weekly_friday_every_2_with_hour() {
        let r = parse("FREQ=WEEKLY;BYDAY=FR;INTERVAL=2;BYHOUR=18").unwrap();
        assert_eq!(r.interval, 2);
        assert_eq!(r.by_day, vec![Weekday::Fri]);
        assert_eq!(r.by_hour, Some(18));
    }

    #[test]
    fn parse_monthly_first() {
        let r = parse("FREQ=MONTHLY;BYMONTHDAY=1").unwrap();
        assert_eq!(r.freq, Freq::Monthly);
        assert_eq!(r.by_month_day, Some(1));
    }

    #[test]
    fn parse_yearly_birthday() {
        let r = parse("FREQ=YEARLY;BYMONTH=3;BYMONTHDAY=15").unwrap();
        assert_eq!(r.freq, Freq::Yearly);
        assert_eq!(r.by_month, Some(3));
        assert_eq!(r.by_month_day, Some(15));
    }

    #[test]
    fn next_daily_tomorrow() {
        let r = parse("FREQ=DAILY").unwrap();
        let n = next_occurrence(&r, dt(2026, 4, 19, 10, 0)).unwrap();
        assert_eq!(n.date_naive(), NaiveDate::from_ymd_opt(2026, 4, 20).unwrap());
    }

    #[test]
    fn next_weekly_friday_from_thursday() {
        let r = parse("FREQ=WEEKLY;BYDAY=FR").unwrap();
        // 2026-04-23 is a Thursday
        let n = next_occurrence(&r, dt(2026, 4, 23, 10, 0)).unwrap();
        assert_eq!(n.date_naive(), NaiveDate::from_ymd_opt(2026, 4, 24).unwrap());
    }

    #[test]
    fn next_weekly_friday_from_friday_returns_next_friday() {
        let r = parse("FREQ=WEEKLY;BYDAY=FR").unwrap();
        // 2026-04-24 is a Friday at 10:00 ; next is 2026-05-01 (next FR)
        let n = next_occurrence(&r, dt(2026, 4, 24, 10, 0)).unwrap();
        assert_eq!(n.date_naive(), NaiveDate::from_ymd_opt(2026, 5, 1).unwrap());
    }

    #[test]
    fn next_monthly_first_from_mid_month() {
        let r = parse("FREQ=MONTHLY;BYMONTHDAY=1").unwrap();
        let n = next_occurrence(&r, dt(2026, 4, 19, 10, 0)).unwrap();
        assert_eq!(n.date_naive(), NaiveDate::from_ymd_opt(2026, 5, 1).unwrap());
    }

    #[test]
    fn next_yearly_birthday() {
        let r = parse("FREQ=YEARLY;BYMONTH=3;BYMONTHDAY=15").unwrap();
        let n = next_occurrence(&r, dt(2026, 4, 19, 10, 0)).unwrap();
        assert_eq!(n.date_naive(), NaiveDate::from_ymd_opt(2027, 3, 15).unwrap());
    }

    #[test]
    fn weekly_with_byhour_snaps_time() {
        let r = parse("FREQ=WEEKLY;BYDAY=MO;BYHOUR=9").unwrap();
        let n = next_occurrence(&r, dt(2026, 4, 19, 18, 0)).unwrap();  // dimanche 18h
        assert_eq!(n.date_naive(), NaiveDate::from_ymd_opt(2026, 4, 20).unwrap());
        assert_eq!(n.hour(), 9);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p grumps-scheduler --target x86_64-pc-windows-msvc recurrence`

Expected: `test result: ok. 11 passed`. If a test fails, fix the implementation (likely off-by-one in matches_rule).

- [ ] **Step 3: Commit**

```bash
git add crates/scheduler/src/recurrence.rs
git commit -m "feat(scheduler): RRULE parser + next_occurrence for 5 launch cases"
```

---

## Task 12 : AgentSession types

**Files:**
- Modify: `crates/scheduler/src/session.rs`

- [ ] **Step 1: Write the type and tests**

Modify `crates/scheduler/src/session.rs` :

```rust
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: String,
    pub member_id: String,
    pub last_message_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub messages: Vec<SessionMessage>,
    pub pending_action: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum SessionMessage {
    User { content: String },
    Assistant { content: String, #[serde(default)] tool_calls: Vec<serde_json::Value> },
    Tool { tool_use_id: String, content: serde_json::Value },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_serializes_role() {
        let m = SessionMessage::User { content: "hi".into() };
        let j = serde_json::to_value(&m).unwrap();
        assert_eq!(j["role"], "user");
        assert_eq!(j["content"], "hi");
    }

    #[test]
    fn assistant_with_no_tool_calls_omits_default() {
        let m = SessionMessage::Assistant { content: "ok".into(), tool_calls: vec![] };
        let j = serde_json::to_string(&m).unwrap();
        // tool_calls is included as empty array — that's fine, just ensure roundtrip
        let back: SessionMessage = serde_json::from_str(&j).unwrap();
        match back {
            SessionMessage::Assistant { content, .. } => assert_eq!(content, "ok"),
            _ => panic!("wrong variant"),
        }
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p grumps-scheduler --target x86_64-pc-windows-msvc session`

Expected: `test result: ok. 2 passed`.

- [ ] **Step 3: Commit**

```bash
git add crates/scheduler/src/session.rs
git commit -m "feat(scheduler): AgentSession + SessionMessage types"
```

---

## Task 13 : Scheduler CRUD on WorkspaceDb (actions + sessions)

**Files:**
- Modify: `crates/worker/Cargo.toml`
- Modify: `crates/worker/src/db.rs`

- [ ] **Step 1: Add grumps-scheduler dep to worker**

Append to `crates/worker/Cargo.toml` `[dependencies]` :

```toml
grumps-scheduler = { path = "../scheduler" }
```

- [ ] **Step 2: Add CRUD methods**

Append to `crates/worker/src/db.rs` :

```rust
// =============================================
// ScheduledAction CRUD (see spec § 5.3 + § 7)
// =============================================

use grumps_scheduler::{ScheduledAction, ActionType, ActionStatus, NewScheduledAction, AgentSession, SessionMessage};

#[derive(Deserialize, Debug, Clone)]
pub struct ScheduledActionRow {
    pub id: String,
    pub action_type: String,
    pub title: String,
    pub trigger_at: String,
    pub recurrence: Option<String>,
    pub condition: Option<String>,
    pub payload: String,
    pub target_chat: String,
    pub status: String,
    pub last_fired_at: Option<String>,
    pub last_error: Option<String>,
    pub fire_count: i64,
    pub created_by: Option<String>,
    pub created_at: String,
}

impl<'a> WorkspaceDb<'a> {
    pub async fn create_scheduled_action(&self, a: &NewScheduledAction) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let action_type = serde_json::to_value(&a.action_type).unwrap()
            .as_str().unwrap_or("reminder").to_string();
        let condition_json: serde_json::Value = a.condition.clone().unwrap_or(serde_json::Value::Null);
        let payload_str = serde_json::to_string(&a.payload).unwrap_or_else(|_| "{}".into());
        let target = a.target_chat.clone().unwrap_or_else(|| "group".into());

        self.q(
            "INSERT INTO scheduled_actions (id, action_type, title, trigger_at, recurrence, condition, payload, target_chat, created_by) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            vec![
                id.clone().into(),
                action_type.into(),
                a.title.clone().into(),
                a.trigger_at.to_rfc3339().into(),
                a.recurrence.clone().map(serde_json::Value::String).unwrap_or(serde_json::Value::Null),
                if condition_json.is_null() { serde_json::Value::Null } else { serde_json::Value::String(condition_json.to_string()) },
                payload_str.into(),
                target.into(),
                a.created_by.clone().map(serde_json::Value::String).unwrap_or(serde_json::Value::Null),
            ],
        ).await?;
        Ok(id)
    }

    pub async fn delete_scheduled_action(&self, id: &str) -> Result<bool> {
        let resp = self.q("DELETE FROM scheduled_actions WHERE id = ?1", vec![id.into()]).await?;
        Ok(resp.meta.as_ref().and_then(|m| m.changes).unwrap_or(0) > 0)
    }

    pub async fn get_scheduled_action(&self, id: &str) -> Result<Option<ScheduledAction>> {
        let resp = self.q(
            "SELECT id, action_type, title, trigger_at, recurrence, condition, payload, target_chat, status, last_fired_at, last_error, fire_count, created_by, created_at \
             FROM scheduled_actions WHERE id = ?1",
            vec![id.into()],
        ).await?;
        let row: Option<ScheduledActionRow> = extract_first(&resp)?;
        Ok(row.map(scheduled_row_to_action))
    }

    pub async fn list_scheduled_actions(&self, status_filter: Option<&str>, limit: i64, offset: i64) -> Result<Vec<ScheduledAction>> {
        let mut sql = String::from(
            "SELECT id, action_type, title, trigger_at, recurrence, condition, payload, target_chat, status, last_fired_at, last_error, fire_count, created_by, created_at \
             FROM scheduled_actions"
        );
        let mut params: Vec<serde_json::Value> = vec![];
        if let Some(s) = status_filter {
            params.push(s.into());
            sql.push_str(&format!(" WHERE status = ?{}", params.len()));
        }
        sql.push_str(" ORDER BY trigger_at ASC");
        params.push(limit.into());
        sql.push_str(&format!(" LIMIT ?{}", params.len()));
        params.push(offset.into());
        sql.push_str(&format!(" OFFSET ?{}", params.len()));
        let resp = self.q(&sql, params).await?;
        let rows: Vec<ScheduledActionRow> = extract_rows(&resp)?;
        Ok(rows.into_iter().map(scheduled_row_to_action).collect())
    }

    pub async fn list_due_actions(&self, now_iso: &str, limit: i64) -> Result<Vec<ScheduledAction>> {
        let resp = self.q(
            "SELECT id, action_type, title, trigger_at, recurrence, condition, payload, target_chat, status, last_fired_at, last_error, fire_count, created_by, created_at \
             FROM scheduled_actions WHERE status = 'pending' AND trigger_at <= ?1 ORDER BY trigger_at ASC LIMIT ?2",
            vec![now_iso.into(), limit.into()],
        ).await?;
        let rows: Vec<ScheduledActionRow> = extract_rows(&resp)?;
        Ok(rows.into_iter().map(scheduled_row_to_action).collect())
    }

    pub async fn next_pending_trigger_at(&self) -> Result<Option<String>> {
        #[derive(Deserialize)]
        struct Row { trigger_at: String }
        let resp = self.q(
            "SELECT trigger_at FROM scheduled_actions WHERE status = 'pending' ORDER BY trigger_at ASC LIMIT 1",
            vec![],
        ).await?;
        let row: Option<Row> = extract_first(&resp)?;
        Ok(row.map(|r| r.trigger_at))
    }

    pub async fn mark_action_firing(&self, id: &str) -> Result<bool> {
        let resp = self.q(
            "UPDATE scheduled_actions SET status='firing' WHERE id=?1 AND status='pending'",
            vec![id.into()],
        ).await?;
        Ok(resp.meta.as_ref().and_then(|m| m.changes).unwrap_or(0) > 0)
    }

    pub async fn mark_action_done(&self, id: &str) -> Result<()> {
        self.q(
            "UPDATE scheduled_actions SET status='done', last_fired_at=datetime('now'), fire_count=fire_count+1 WHERE id=?1",
            vec![id.into()],
        ).await?;
        Ok(())
    }

    pub async fn reschedule_action(&self, id: &str, next_trigger_at: &str) -> Result<()> {
        self.q(
            "UPDATE scheduled_actions SET status='pending', trigger_at=?1, last_fired_at=datetime('now'), fire_count=fire_count+1 WHERE id=?2",
            vec![next_trigger_at.into(), id.into()],
        ).await?;
        Ok(())
    }

    pub async fn mark_action_failed(&self, id: &str, error: &str) -> Result<()> {
        self.q(
            "UPDATE scheduled_actions SET status='failed', last_error=?1, last_fired_at=datetime('now'), fire_count=fire_count+1 WHERE id=?2",
            vec![error.into(), id.into()],
        ).await?;
        Ok(())
    }
}

fn scheduled_row_to_action(r: ScheduledActionRow) -> ScheduledAction {
    use chrono::{DateTime, Utc, TimeZone};
    let parse_dt = |s: &str| -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc.timestamp_opt(0, 0).unwrap())
    };
    ScheduledAction {
        id: r.id,
        action_type: serde_json::from_value(serde_json::Value::String(r.action_type.clone()))
            .unwrap_or(ActionType::Reminder),
        title: r.title,
        trigger_at: parse_dt(&r.trigger_at),
        recurrence: r.recurrence,
        condition: r.condition.as_deref().and_then(|s| serde_json::from_str(s).ok()),
        payload: serde_json::from_str(&r.payload).unwrap_or(serde_json::Value::Null),
        target_chat: r.target_chat,
        status: serde_json::from_value(serde_json::Value::String(r.status.clone()))
            .unwrap_or(ActionStatus::Pending),
        last_fired_at: r.last_fired_at.as_deref().map(parse_dt),
        last_error: r.last_error,
        fire_count: r.fire_count,
        created_by: r.created_by,
        created_at: parse_dt(&r.created_at),
    }
}

// =============================================
// AgentSession CRUD (for Plan B — minimal here)
// =============================================

impl<'a> WorkspaceDb<'a> {
    pub async fn upsert_agent_session(&self, member_id: &str, messages: &[SessionMessage], pending: Option<&serde_json::Value>) -> Result<String> {
        // Look up active session for member
        #[derive(Deserialize)]
        struct Row { id: String }
        let row: Option<Row> = extract_first(&self.q(
            "SELECT id FROM agent_sessions WHERE member_id=?1 AND expires_at > datetime('now') LIMIT 1",
            vec![member_id.into()],
        ).await?)?;

        let messages_json = serde_json::to_string(messages).unwrap_or_else(|_| "[]".into());
        let pending_json: serde_json::Value = pending.cloned().unwrap_or(serde_json::Value::Null);

        match row {
            Some(r) => {
                self.q(
                    "UPDATE agent_sessions SET messages=?1, pending_action=?2, last_message_at=datetime('now'), expires_at=datetime('now', '+60 minutes') WHERE id=?3",
                    vec![messages_json.into(), pending_json, r.id.clone().into()],
                ).await?;
                Ok(r.id)
            }
            None => {
                let id = uuid::Uuid::new_v4().to_string();
                self.q(
                    "INSERT INTO agent_sessions (id, member_id, last_message_at, expires_at, messages, pending_action) \
                     VALUES (?1, ?2, datetime('now'), datetime('now', '+60 minutes'), ?3, ?4)",
                    vec![id.clone().into(), member_id.into(), messages_json.into(), pending_json],
                ).await?;
                Ok(id)
            }
        }
    }

    pub async fn get_active_agent_session(&self, member_id: &str) -> Result<Option<AgentSession>> {
        #[derive(Deserialize)]
        struct Row {
            id: String, member_id: String, last_message_at: String, expires_at: String,
            messages: String, pending_action: Option<String>, created_at: String,
        }
        let resp = self.q(
            "SELECT id, member_id, last_message_at, expires_at, messages, pending_action, created_at FROM agent_sessions \
             WHERE member_id=?1 AND expires_at > datetime('now') LIMIT 1",
            vec![member_id.into()],
        ).await?;
        let row: Option<Row> = extract_first(&resp)?;
        Ok(row.map(|r| {
            use chrono::{DateTime, Utc, TimeZone};
            let parse_dt = |s: &str| DateTime::parse_from_rfc3339(s)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc.timestamp_opt(0, 0).unwrap());
            AgentSession {
                id: r.id,
                member_id: r.member_id,
                last_message_at: parse_dt(&r.last_message_at),
                expires_at: parse_dt(&r.expires_at),
                messages: serde_json::from_str(&r.messages).unwrap_or_default(),
                pending_action: r.pending_action.as_deref().and_then(|s| serde_json::from_str(s).ok()),
                created_at: parse_dt(&r.created_at),
            }
        }))
    }
}
```

- [ ] **Step 3: Verify compiles**

Run: `cargo check -p grumps-worker --target x86_64-pc-windows-msvc`

Expected: zero errors.

- [ ] **Step 4: Commit**

```bash
git add crates/worker/Cargo.toml crates/worker/src/db.rs
git commit -m "feat(scheduler): WorkspaceDb CRUD for scheduled_actions + agent_sessions"
```

---

## Task 14 : Wrangler config — Vectorize, Workers AI, DO bindings

**Files:**
- Modify: `wrangler.toml`

- [ ] **Step 1: Append new bindings**

Modify `wrangler.toml`. Add at the bottom :

```toml
[ai]
binding = "AI"

[[vectorize]]
binding = "CHAT_RAG"
index_name = "grumps-chat-rag"

[[durable_objects.bindings]]
name = "WS_SCHEDULER"
class_name = "WorkspaceScheduler"

[[migrations]]
tag = "v2"
new_sqlite_classes = ["WorkspaceScheduler"]

[vars]
WEB_SEARCH_PROVIDER = "brave"     # Plan D introduces the actual usage
ENVIRONMENT = "development"
WA_PHONE_NUMBER_ID = "1007990222407676"
WA_VERIFY_TOKEN = "grumps_verify_2026"
TG_BOT_USERNAME = "HeyGrumpsBot"
```

> Note : `[vars]` already exists in `wrangler.toml` — preserve existing values, just add `WEB_SEARCH_PROVIDER`. The other bindings are new.

- [ ] **Step 2: Create the Vectorize index (one-shot)**

Run :
```bash
wrangler vectorize create grumps-chat-rag --dimensions=1024 --metric=cosine
```

Expected output : `✨ Successfully created index 'grumps-chat-rag'`. If it already exists, expected error mentions duplicate — safe to ignore in this step.

- [ ] **Step 3: Verify wrangler.toml parses**

Run: `wrangler types` (which validates the config and generates types)

Expected: succeeds, no parse errors.

- [ ] **Step 4: Commit**

```bash
git add wrangler.toml
git commit -m "feat(infra): wrangler bindings for Vectorize, Workers AI, DO scheduler"
```

---

## Task 15 : Durable Object — `WorkspaceScheduler` skeleton

**Files:**
- Create: `crates/worker/src/durable_objects/mod.rs`
- Create: `crates/worker/src/durable_objects/scheduler.rs`
- Modify: `crates/worker/src/lib.rs`

- [ ] **Step 1: Create the module structure**

Create `crates/worker/src/durable_objects/mod.rs` :

```rust
pub mod scheduler;
pub use scheduler::WorkspaceScheduler;
```

- [ ] **Step 2: Implement the DO skeleton**

Create `crates/worker/src/durable_objects/scheduler.rs` :

```rust
//! WorkspaceScheduler : 1 instance per workspace.
//! Holds the next-due-alarm via state.storage().set_alarm().
//! See spec § 7.

use worker::*;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ScheduleRpc {
    /// Schedule a new action — DO updates alarm if earlier than current.
    Schedule { trigger_at: String },
    /// Force recompute next alarm from D1 (e.g. after delete/cancel).
    Reschedule,
    /// Clear the alarm (e.g. last action cancelled).
    Clear,
}

#[durable_object]
pub struct WorkspaceScheduler {
    state: State,
    env: Env,
}

#[durable_object]
impl DurableObject for WorkspaceScheduler {
    fn new(state: State, env: Env) -> Self { Self { state, env } }

    async fn fetch(&mut self, mut req: Request) -> Result<Response> {
        let body: ScheduleRpc = req.json().await
            .map_err(|e| Error::RustError(format!("bad rpc body: {e}")))?;
        match body {
            ScheduleRpc::Schedule { trigger_at } => {
                let new_at = parse_iso(&trigger_at)?;
                let current = self.state.storage().get_alarm().await?;
                let new_ms = new_at.timestamp_millis();
                let should_update = current.map(|c| new_ms < c).unwrap_or(true);
                if should_update {
                    self.state.storage().set_alarm(new_ms).await?;
                }
                Response::ok("scheduled")
            }
            ScheduleRpc::Reschedule => {
                self.recompute_alarm().await?;
                Response::ok("rescheduled")
            }
            ScheduleRpc::Clear => {
                self.state.storage().delete_alarm().await?;
                Response::ok("cleared")
            }
        }
    }

    async fn alarm(&mut self) -> Result<Response> {
        // Implementation in Task 17
        // For now, log and re-arm via recompute
        console_log!("WorkspaceScheduler alarm fired (skeleton, no-op)");
        self.recompute_alarm().await?;
        Response::ok("fired")
    }
}

impl WorkspaceScheduler {
    async fn recompute_alarm(&mut self) -> Result<()> {
        // Lookup workspace_slug from DO id (id_from_name was used)
        let slug = self.state.id().name().unwrap_or_default();
        if slug.is_empty() {
            console_log!("WorkspaceScheduler: empty slug, skipping recompute");
            return Ok(());
        }
        // Resolve D1 database for this workspace via Index DB
        let next_iso = match resolve_next_pending(&self.env, &slug).await {
            Ok(opt) => opt,
            Err(e) => { console_log!("recompute_alarm error: {e}"); return Ok(()); }
        };
        match next_iso {
            Some(iso) => {
                let dt = parse_iso(&iso)?;
                self.state.storage().set_alarm(dt.timestamp_millis()).await?;
            }
            None => {
                self.state.storage().delete_alarm().await?;
            }
        }
        Ok(())
    }
}

fn parse_iso(s: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| Error::RustError(format!("bad iso datetime '{s}': {e}")))
}

async fn resolve_next_pending(env: &Env, slug: &str) -> Result<Option<String>> {
    use crate::db::{get_index_db, lookup_workspace_by_slug, WorkspaceDb};
    use crate::d1_rest::D1RestClient;
    let index = get_index_db(env)?;
    let ws = lookup_workspace_by_slug(&index, slug).await?
        .ok_or_else(|| Error::RustError(format!("workspace not found: {slug}")))?;
    let cf_account = env.secret("CF_ACCOUNT_ID")?.to_string();
    let cf_token = env.secret("CF_API_TOKEN")?.to_string();
    let client = D1RestClient::new(cf_account, cf_token);
    let db = WorkspaceDb::new(&client, ws.d1_database_id);
    db.next_pending_trigger_at().await
}
```

- [ ] **Step 3: Wire DO export in lib.rs**

Modify `crates/worker/src/lib.rs`. Add `mod durable_objects;` near the top with other `mod` declarations, and add a public re-export so the DO is visible to wrangler. After the `mod` block, append :

```rust
pub use durable_objects::WorkspaceScheduler;
```

- [ ] **Step 4: Verify compiles**

Run: `cargo check -p grumps-worker --target wasm32-unknown-unknown`

Expected: zero errors. (Note : we use wasm32 target for the actual build, native target for tests.)

If `wasm32-unknown-unknown` toolchain is not installed : `rustup target add wasm32-unknown-unknown`.

- [ ] **Step 5: Commit**

```bash
git add crates/worker/src/durable_objects crates/worker/src/lib.rs
git commit -m "feat(scheduler): WorkspaceScheduler Durable Object skeleton"
```

---

## Task 16 : Scheduler executor (reminder + event_notify + recap)

**Files:**
- Create: `crates/worker/src/scheduler_executor.rs`
- Modify: `crates/worker/src/lib.rs`

- [ ] **Step 1: Create the executor**

Create `crates/worker/src/scheduler_executor.rs` :

```rust
//! Dispatch scheduled actions when their alarm fires.
//! See spec § 7.4.
//!
//! Plan A scope : reminder, event_notify, recap.
//! follow_up + agent_task come in Plan B (require agent loop).

use worker::*;
use grumps_scheduler::{ScheduledAction, ActionType};
use crate::db::{WorkspaceDb, get_index_db, lookup_workspace_by_slug};
use crate::d1_rest::D1RestClient;
use crate::recurrence;

pub async fn execute_action(env: &Env, ws_slug: &str, action: &ScheduledAction) -> Result<()> {
    let index = get_index_db(env)?;
    let ws = lookup_workspace_by_slug(&index, ws_slug).await?
        .ok_or_else(|| Error::RustError(format!("workspace not found: {ws_slug}")))?;
    let cf_account = env.secret("CF_ACCOUNT_ID")?.to_string();
    let cf_token = env.secret("CF_API_TOKEN")?.to_string();
    let client = D1RestClient::new(cf_account, cf_token);
    let db = WorkspaceDb::new(&client, ws.d1_database_id.clone());

    let send_result = match action.action_type {
        ActionType::Reminder => execute_reminder(env, &ws, action).await,
        ActionType::EventNotify => execute_event_notify(env, &ws, &db, action).await,
        ActionType::Recap => execute_recap(env, &ws, &db, action).await,
        ActionType::FollowUp | ActionType::AgentTask => {
            // Plan B will fill these
            console_log!("action_type {:?} not yet implemented in Plan A", action.action_type);
            Ok(())
        }
    };

    // Mark done (or re-schedule if recurrent)
    match send_result {
        Ok(()) => {
            if let Some(rrule) = &action.recurrence {
                let parsed = recurrence::parse(rrule)
                    .map_err(|e| Error::RustError(format!("bad rrule: {e}")))?;
                if let Some(next) = recurrence::next_occurrence(&parsed, action.trigger_at) {
                    db.reschedule_action(&action.id, &next.to_rfc3339()).await?;
                } else {
                    db.mark_action_done(&action.id).await?;
                }
            } else {
                db.mark_action_done(&action.id).await?;
            }
        }
        Err(e) => {
            db.mark_action_failed(&action.id, &e.to_string()).await?;
        }
    }
    Ok(())
}

async fn execute_reminder(env: &Env, ws: &crate::db::WorkspaceMetaRow, action: &ScheduledAction) -> Result<()> {
    let text = action.payload.get("text").and_then(|v| v.as_str()).unwrap_or(&action.title);
    let body = format!("⏰ Rappel : {text}");
    send_to_group(env, ws, &body).await
}

async fn execute_event_notify(env: &Env, ws: &crate::db::WorkspaceMetaRow, db: &WorkspaceDb<'_>, action: &ScheduledAction) -> Result<()> {
    let event_id = action.payload.get("event_id").and_then(|v| v.as_str())
        .ok_or_else(|| Error::RustError("event_notify missing event_id".into()))?;
    let lead = action.payload.get("lead_minutes").and_then(|v| v.as_i64()).unwrap_or(15);
    let event = db.get_event(event_id).await?
        .ok_or_else(|| Error::RustError(format!("event not found: {event_id}")))?;
    let body = format!("📅 Dans {lead}min : {} ({})", event.title,
        event.location.as_deref().unwrap_or("lieu non précisé"));
    send_to_group(env, ws, &body).await
}

async fn execute_recap(env: &Env, ws: &crate::db::WorkspaceMetaRow, _db: &WorkspaceDb<'_>, _action: &ScheduledAction) -> Result<()> {
    // Reuse existing recap logic from worker/src/cron.rs if it exists.
    // For Plan A, we send a placeholder recap. Plan B will integrate real LLM-generated recap.
    let body = "📋 Recap hebdomadaire — placeholder (Plan B improves this with LLM-generated content).".to_string();
    send_to_group(env, ws, &body).await
}

async fn send_to_group(env: &Env, ws: &crate::db::WorkspaceMetaRow, body: &str) -> Result<()> {
    // Reuse messaging dispatch from existing code.
    // Look up platform/channel from workspaces_meta and dispatch via the right adapter.
    // For Plan A simplicity, route via the existing helper if any. If not, use messaging crate directly.
    use grumps_messaging::adapter::OutboundMessage;
    let out = OutboundMessage { text: body.to_string(), reply_to: None };
    // The actual platform send is handled by an existing function in the worker
    // (look for crate::messaging::send_outbound or similar).
    // If no such helper exists, instantiate the platform adapter here based on ws.platform.
    crate::messaging_dispatch::send_to_workspace(env, &ws.slug, &out).await
}
```

- [ ] **Step 2: Wire module in lib.rs**

In `crates/worker/src/lib.rs`, add `mod scheduler_executor;` to the `mod` declarations (near `mod cron;`).

- [ ] **Step 3: Check if `messaging_dispatch::send_to_workspace` exists**

Run: `grep -r "send_to_workspace\|fn send_outbound\|send_message_to_group" crates/worker/src/ | head -5`

If it doesn't exist, create a thin helper. Add to `crates/worker/src/lib.rs` :

```rust
mod messaging_dispatch;
```

Create `crates/worker/src/messaging_dispatch.rs` :

```rust
//! Thin helper to send a message to a workspace's chat group.
//! Resolves platform from Index DB, builds adapter, sends.

use worker::*;
use grumps_messaging::adapter::OutboundMessage;
use crate::db::{get_index_db};

pub async fn send_to_workspace(env: &Env, ws_slug: &str, out: &OutboundMessage) -> Result<()> {
    use serde::Deserialize;
    #[derive(Deserialize)]
    struct Row { platform: String, platform_channel_id: String }
    let index = get_index_db(env)?;
    let row: Option<Row> = index.prepare(
        "SELECT platform, platform_channel_id FROM workspaces_meta WHERE slug = ?1"
    ).bind(&[ws_slug.into()])?.first(None).await?;
    let row = row.ok_or_else(|| Error::RustError(format!("workspace not found: {ws_slug}")))?;
    match row.platform.as_str() {
        "telegram" => {
            let token = env.secret("TG_BOT_TOKEN")?.to_string();
            let body = serde_json::json!({
                "chat_id": row.platform_channel_id,
                "text": out.text,
                "parse_mode": "Markdown",
            });
            let url = format!("https://api.telegram.org/bot{token}/sendMessage");
            let mut headers = Headers::new();
            headers.set("content-type", "application/json")?;
            let req = Request::new_with_init(&url, RequestInit::new()
                .with_method(Method::Post)
                .with_headers(headers)
                .with_body(Some(serde_json::to_string(&body).unwrap().into())))?;
            Fetch::Request(req).send().await?;
            Ok(())
        }
        "whatsapp" => {
            // Reuse existing WA send helper if it exists in routes/webhook.rs ;
            // if not, replicate the call to https://graph.facebook.com/.../messages.
            // For Plan A keep this minimal — a stub that logs is OK as long as
            // tests use telegram.
            console_log!("WA send not implemented in messaging_dispatch (Plan A) ; use existing handler path");
            Err(Error::RustError("WA send via messaging_dispatch not yet implemented".into()))
        }
        "discord" => {
            console_log!("Discord send not implemented in messaging_dispatch (Plan A)");
            Err(Error::RustError("Discord send via messaging_dispatch not yet implemented".into()))
        }
        other => Err(Error::RustError(format!("unknown platform: {other}"))),
    }
}
```

- [ ] **Step 4: Verify compiles**

Run: `cargo check -p grumps-worker --target wasm32-unknown-unknown`

Expected: zero errors.

- [ ] **Step 5: Commit**

```bash
git add crates/worker/src/scheduler_executor.rs crates/worker/src/messaging_dispatch.rs crates/worker/src/lib.rs
git commit -m "feat(scheduler): executor dispatch + messaging helper for reminder/event_notify/recap"
```

---

## Task 17 : Wire executor into DO `alarm()` handler

**Files:**
- Modify: `crates/worker/src/durable_objects/scheduler.rs`

- [ ] **Step 1: Implement `alarm()` properly**

Modify `crates/worker/src/durable_objects/scheduler.rs`, replacing the `alarm()` method with :

```rust
async fn alarm(&mut self) -> Result<Response> {
    let slug = self.state.id().name().unwrap_or_default();
    if slug.is_empty() {
        console_log!("WorkspaceScheduler.alarm: empty slug");
        return Response::ok("noop");
    }
    let now = chrono::Utc::now().to_rfc3339();
    let due = match resolve_due_actions(&self.env, &slug, &now).await {
        Ok(v) => v,
        Err(e) => { console_log!("alarm: resolve_due_actions error: {e}"); return Response::ok("noop"); }
    };
    for action in &due {
        // Lock first
        let locked = match resolve_lock_action(&self.env, &slug, &action.id).await {
            Ok(b) => b, Err(e) => { console_log!("lock error: {e}"); false }
        };
        if !locked { continue; }                           // someone else got it
        match crate::scheduler_executor::execute_action(&self.env, &slug, action).await {
            Ok(()) => console_log!("executed action {}", action.id),
            Err(e) => console_log!("execute_action error for {}: {e}", action.id),
        }
    }
    // Re-arm next
    if let Err(e) = self.recompute_alarm().await {
        console_log!("recompute_alarm error: {e}");
    }
    Response::ok("fired")
}
```

Add the supporting helpers at the bottom of the file :

```rust
async fn resolve_due_actions(env: &Env, slug: &str, now_iso: &str) -> Result<Vec<grumps_scheduler::ScheduledAction>> {
    use crate::db::{get_index_db, lookup_workspace_by_slug, WorkspaceDb};
    use crate::d1_rest::D1RestClient;
    let index = get_index_db(env)?;
    let ws = lookup_workspace_by_slug(&index, slug).await?
        .ok_or_else(|| Error::RustError(format!("workspace not found: {slug}")))?;
    let cf_account = env.secret("CF_ACCOUNT_ID")?.to_string();
    let cf_token = env.secret("CF_API_TOKEN")?.to_string();
    let client = D1RestClient::new(cf_account, cf_token);
    let db = WorkspaceDb::new(&client, ws.d1_database_id);
    db.list_due_actions(now_iso, 50).await
}

async fn resolve_lock_action(env: &Env, slug: &str, action_id: &str) -> Result<bool> {
    use crate::db::{get_index_db, lookup_workspace_by_slug, WorkspaceDb};
    use crate::d1_rest::D1RestClient;
    let index = get_index_db(env)?;
    let ws = lookup_workspace_by_slug(&index, slug).await?
        .ok_or_else(|| Error::RustError(format!("workspace not found: {slug}")))?;
    let cf_account = env.secret("CF_ACCOUNT_ID")?.to_string();
    let cf_token = env.secret("CF_API_TOKEN")?.to_string();
    let client = D1RestClient::new(cf_account, cf_token);
    let db = WorkspaceDb::new(&client, ws.d1_database_id);
    db.mark_action_firing(action_id).await
}
```

- [ ] **Step 2: Verify compiles**

Run: `cargo check -p grumps-worker --target wasm32-unknown-unknown`

Expected: zero errors.

- [ ] **Step 3: Commit**

```bash
git add crates/worker/src/durable_objects/scheduler.rs
git commit -m "feat(scheduler): DO alarm handler — lock + execute + re-arm"
```

---

## Task 18 : REST routes — memory CRUD

**Files:**
- Create: `crates/worker/src/routes/util.rs`  *(query param parser, ~15 lignes, partagé entre les 3 prochaines tâches)*
- Create: `crates/worker/src/routes/memory.rs`
- Modify: `crates/worker/src/routes/mod.rs`
- Modify: `crates/worker/src/lib.rs`

- [ ] **Step 0: Create the shared query helper (no external dep)**

Create `crates/worker/src/routes/util.rs` :

```rust
//! Tiny query-string helpers — avoid pulling serde_urlencoded.

use worker::Url;

/// Read query params from a URL into a struct of choice.
/// Caller passes a closure that pattern-matches on (key, value) pairs.
pub fn read_query<F>(url: &Url, mut f: F)
where F: FnMut(&str, &str)
{
    for (k, v) in url.query_pairs() {
        f(k.as_ref(), v.as_ref());
    }
}
```

In `crates/worker/src/routes/mod.rs`, add :
```rust
pub mod util;
```

- [ ] **Step 1: Create the route handlers**

Create `crates/worker/src/routes/memory.rs` :

```rust
//! REST routes for memory_entries (workspace-scoped, JWT-auth).
//! See spec § 18.

use worker::*;
use serde::Deserialize;
use crate::middleware::{auth_required, build_workspace_db};
use crate::error::respond_error;
use crate::routes::util::read_query;

pub async fn list(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match auth_required(&req, &ctx.env) { Ok(c) => c, Err(r) => return Ok(r) };
    let slug = ctx.param("slug").map(|s| s.as_str()).unwrap_or("");
    if !claims.workspaces.iter().any(|w| w == slug) {
        return respond_error(403, "forbidden", "not a member of this workspace");
    }
    let url = req.url()?;
    let mut kind = None;
    let mut source = None;
    let mut limit = 50i64;
    let mut offset = 0i64;
    read_query(&url, |k, v| match k {
        "kind" => kind = Some(v.to_string()),
        "source" => source = Some(v.to_string()),
        "limit" => { limit = v.parse().unwrap_or(50).clamp(1, 200); }
        "offset" => { offset = v.parse().unwrap_or(0).max(0); }
        _ => {}
    });
    let db = build_workspace_db(&ctx.env, slug).await?;
    let entries = db.list_memory(kind.as_deref(), source.as_deref(), limit, offset).await?;
    Response::from_json(&entries)
}

#[derive(Deserialize)]
struct CreateBody {
    key: Option<String>,
    value: String,
    kind: grumps_memory::MemoryKind,
    related_member: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    pinned: Option<bool>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match auth_required(&req, &ctx.env) { Ok(c) => c, Err(r) => return Ok(r) };
    let slug = ctx.param("slug").map(|s| s.as_str()).unwrap_or("");
    if !claims.workspaces.iter().any(|w| w == slug) {
        return respond_error(403, "forbidden", "not a member of this workspace");
    }
    let body: CreateBody = req.json().await
        .map_err(|e| Error::RustError(format!("bad body: {e}")))?;
    let db = build_workspace_db(&ctx.env, slug).await?;
    let new = grumps_memory::NewMemoryEntry {
        key: body.key,
        value: body.value,
        kind: body.kind,
        related_member: body.related_member,
        tags: body.tags,
        source: grumps_memory::MemorySource::Web,
        confidence: Some(1.0),
        pinned: body.pinned,
        expires_at: body.expires_at,
        created_by: Some(claims.sub.clone()),
    };
    let id = db.create_memory(&new).await?;
    let entry = db.get_memory(&id).await?;
    Response::from_json(&entry).map(|r| r.with_status(201))
}

pub async fn get(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match auth_required(&req, &ctx.env) { Ok(c) => c, Err(r) => return Ok(r) };
    let slug = ctx.param("slug").map(|s| s.as_str()).unwrap_or("");
    if !claims.workspaces.iter().any(|w| w == slug) {
        return respond_error(403, "forbidden", "not a member of this workspace");
    }
    let id = ctx.param("id").map(|s| s.as_str()).unwrap_or("");
    let db = build_workspace_db(&ctx.env, slug).await?;
    match db.get_memory(id).await? {
        Some(e) => Response::from_json(&e),
        None => respond_error(404, "not_found", "memory entry not found"),
    }
}

#[derive(Deserialize)]
struct UpdateBody {
    value: Option<String>,
    pinned: Option<bool>,
    expires_at: Option<String>,
}

pub async fn update(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match auth_required(&req, &ctx.env) { Ok(c) => c, Err(r) => return Ok(r) };
    let slug = ctx.param("slug").map(|s| s.as_str()).unwrap_or("");
    if !claims.workspaces.iter().any(|w| w == slug) {
        return respond_error(403, "forbidden", "not a member of this workspace");
    }
    let id = ctx.param("id").map(|s| s.as_str()).unwrap_or("");
    let body: UpdateBody = req.json().await
        .map_err(|e| Error::RustError(format!("bad body: {e}")))?;
    let db = build_workspace_db(&ctx.env, slug).await?;
    let updated = db.update_memory(id, body.value.as_deref(), body.pinned, body.expires_at.as_deref()).await?;
    if !updated { return respond_error(404, "not_found", "memory entry not found"); }
    let entry = db.get_memory(id).await?;
    Response::from_json(&entry)
}

pub async fn delete(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match auth_required(&req, &ctx.env) { Ok(c) => c, Err(r) => return Ok(r) };
    let slug = ctx.param("slug").map(|s| s.as_str()).unwrap_or("");
    if !claims.workspaces.iter().any(|w| w == slug) {
        return respond_error(403, "forbidden", "not a member of this workspace");
    }
    let id = ctx.param("id").map(|s| s.as_str()).unwrap_or("");
    let db = build_workspace_db(&ctx.env, slug).await?;
    let deleted = db.delete_memory(id).await?;
    if !deleted { return respond_error(404, "not_found", "memory entry not found"); }
    Response::empty().map(|r| r.with_status(204))
}
```

- [ ] **Step 2: Add module export**

Modify `crates/worker/src/routes/mod.rs`. Add :

```rust
pub mod memory;
```

If `auth_required`, `build_workspace_db`, `respond_error` don't yet exist with those exact names, check existing patterns. They likely live in `middleware.rs` and `error.rs`. If their names differ, adapt the imports to match what's there.

- [ ] **Step 3: Wire routes in lib.rs**

Modify `crates/worker/src/lib.rs` `Router::new()` block, add :

```rust
.get_async("/api/w/:slug/memory", routes::memory::list)
.post_async("/api/w/:slug/memory", routes::memory::create)
.get_async("/api/w/:slug/memory/:id", routes::memory::get)
.put_async("/api/w/:slug/memory/:id", routes::memory::update)
.delete_async("/api/w/:slug/memory/:id", routes::memory::delete)
```

- [ ] **Step 4: Verify compiles**

Run: `cargo check -p grumps-worker --target wasm32-unknown-unknown`

Expected: zero errors.

- [ ] **Step 5: Commit**

```bash
git add crates/worker/src/routes/memory.rs crates/worker/src/routes/mod.rs crates/worker/src/lib.rs
git commit -m "feat(memory): REST routes for memory CRUD"
```

---

## Task 19 : REST routes — events CRUD

**Files:**
- Create: `crates/worker/src/routes/events.rs`
- Modify: `crates/worker/src/routes/mod.rs`
- Modify: `crates/worker/src/lib.rs`

- [ ] **Step 1: Create the route handlers**

Create `crates/worker/src/routes/events.rs` (mirror the structure of `memory.rs`) :

```rust
//! REST routes for events.

use worker::*;
use serde::Deserialize;
use crate::middleware::{auth_required, build_workspace_db};
use crate::error::respond_error;
use crate::routes::util::read_query;

pub async fn list(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match auth_required(&req, &ctx.env) { Ok(c) => c, Err(r) => return Ok(r) };
    let slug = ctx.param("slug").map(|s| s.as_str()).unwrap_or("");
    if !claims.workspaces.iter().any(|w| w == slug) {
        return respond_error(403, "forbidden", "not a member of this workspace");
    }
    let url = req.url()?;
    let mut from_q: Option<String> = None;
    let mut to_q: Option<String> = None;
    read_query(&url, |k, v| match k {
        "from" => from_q = Some(v.to_string()),
        "to" => to_q = Some(v.to_string()),
        _ => {}
    });
    let now = chrono::Utc::now();
    let from = from_q.unwrap_or_else(|| (now - chrono::Duration::days(30)).to_rfc3339());
    let to = to_q.unwrap_or_else(|| (now + chrono::Duration::days(60)).to_rfc3339());
    let db = build_workspace_db(&ctx.env, slug).await?;
    let events = db.list_events_in_range(&from, &to).await?;
    Response::from_json(&events)
}

#[derive(Deserialize)]
struct CreateBody {
    title: String,
    description: Option<String>,
    starts_at: chrono::DateTime<chrono::Utc>,
    ends_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    all_day: bool,
    location: Option<String>,
    recurrence: Option<String>,
    #[serde(default)]
    attendees: Vec<String>,
    color: Option<String>,
}

pub async fn create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match auth_required(&req, &ctx.env) { Ok(c) => c, Err(r) => return Ok(r) };
    let slug = ctx.param("slug").map(|s| s.as_str()).unwrap_or("");
    if !claims.workspaces.iter().any(|w| w == slug) {
        return respond_error(403, "forbidden", "not a member of this workspace");
    }
    let body: CreateBody = req.json().await
        .map_err(|e| Error::RustError(format!("bad body: {e}")))?;
    let db = build_workspace_db(&ctx.env, slug).await?;
    let new = grumps_calendar::NewEvent {
        title: body.title,
        description: body.description,
        starts_at: body.starts_at,
        ends_at: body.ends_at,
        all_day: body.all_day,
        location: body.location,
        recurrence: body.recurrence,
        attendees: body.attendees,
        color: body.color,
        source: grumps_calendar::EventSource::Web,
        related_todo_id: None,
        created_by: Some(claims.sub.clone()),
    };
    let id = db.create_event(&new).await?;
    let event = db.get_event(&id).await?;
    Response::from_json(&event).map(|r| r.with_status(201))
}

pub async fn get(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match auth_required(&req, &ctx.env) { Ok(c) => c, Err(r) => return Ok(r) };
    let slug = ctx.param("slug").map(|s| s.as_str()).unwrap_or("");
    if !claims.workspaces.iter().any(|w| w == slug) {
        return respond_error(403, "forbidden", "not a member of this workspace");
    }
    let id = ctx.param("id").map(|s| s.as_str()).unwrap_or("");
    let db = build_workspace_db(&ctx.env, slug).await?;
    match db.get_event(id).await? {
        Some(e) => Response::from_json(&e),
        None => respond_error(404, "not_found", "event not found"),
    }
}

#[derive(Deserialize)]
struct UpdateBody {
    title: Option<String>,
    starts_at: Option<String>,
    ends_at: Option<String>,
    location: Option<String>,
}

pub async fn update(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match auth_required(&req, &ctx.env) { Ok(c) => c, Err(r) => return Ok(r) };
    let slug = ctx.param("slug").map(|s| s.as_str()).unwrap_or("");
    if !claims.workspaces.iter().any(|w| w == slug) {
        return respond_error(403, "forbidden", "not a member of this workspace");
    }
    let id = ctx.param("id").map(|s| s.as_str()).unwrap_or("");
    let body: UpdateBody = req.json().await
        .map_err(|e| Error::RustError(format!("bad body: {e}")))?;
    let db = build_workspace_db(&ctx.env, slug).await?;
    let updated = db.update_event(id, body.title.as_deref(), body.starts_at.as_deref(), body.ends_at.as_deref(), body.location.as_deref()).await?;
    if !updated { return respond_error(404, "not_found", "event not found"); }
    let event = db.get_event(id).await?;
    Response::from_json(&event)
}

pub async fn delete(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match auth_required(&req, &ctx.env) { Ok(c) => c, Err(r) => return Ok(r) };
    let slug = ctx.param("slug").map(|s| s.as_str()).unwrap_or("");
    if !claims.workspaces.iter().any(|w| w == slug) {
        return respond_error(403, "forbidden", "not a member of this workspace");
    }
    let id = ctx.param("id").map(|s| s.as_str()).unwrap_or("");
    let db = build_workspace_db(&ctx.env, slug).await?;
    let deleted = db.delete_event(id).await?;
    if !deleted { return respond_error(404, "not_found", "event not found"); }
    Response::empty().map(|r| r.with_status(204))
}
```

- [ ] **Step 2: Add module export**

In `crates/worker/src/routes/mod.rs` :
```rust
pub mod events;
```

- [ ] **Step 3: Wire routes**

Append to `crates/worker/src/lib.rs` `Router::new()` :

```rust
.get_async("/api/w/:slug/events", routes::events::list)
.post_async("/api/w/:slug/events", routes::events::create)
.get_async("/api/w/:slug/events/:id", routes::events::get)
.put_async("/api/w/:slug/events/:id", routes::events::update)
.delete_async("/api/w/:slug/events/:id", routes::events::delete)
```

- [ ] **Step 4: Verify compiles**

Run: `cargo check -p grumps-worker --target wasm32-unknown-unknown`

Expected: zero errors.

- [ ] **Step 5: Commit**

```bash
git add crates/worker/src/routes/events.rs crates/worker/src/routes/mod.rs crates/worker/src/lib.rs
git commit -m "feat(calendar): REST routes for events CRUD"
```

---

## Task 20 : REST routes — scheduled actions (with DO RPC + retry/rollback)

**Files:**
- Create: `crates/worker/src/routes/scheduled.rs`
- Modify: `crates/worker/src/routes/mod.rs`
- Modify: `crates/worker/src/lib.rs`

- [ ] **Step 1: Create the route handlers**

Create `crates/worker/src/routes/scheduled.rs` :

```rust
//! REST routes for scheduled_actions.
//! Create path also RPCs the DO to arm the alarm. Rollback D1 on RPC failure.
//! See spec § 7.3.

use worker::*;
use serde::Deserialize;
use crate::middleware::{auth_required, build_workspace_db};
use crate::error::respond_error;
use crate::routes::util::read_query;

pub async fn list(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match auth_required(&req, &ctx.env) { Ok(c) => c, Err(r) => return Ok(r) };
    let slug = ctx.param("slug").map(|s| s.as_str()).unwrap_or("");
    if !claims.workspaces.iter().any(|w| w == slug) {
        return respond_error(403, "forbidden", "not a member of this workspace");
    }
    let url = req.url()?;
    let mut status = None;
    let mut limit = 50i64;
    let mut offset = 0i64;
    read_query(&url, |k, v| match k {
        "status" => status = Some(v.to_string()),
        "limit" => { limit = v.parse().unwrap_or(50).clamp(1, 200); }
        "offset" => { offset = v.parse().unwrap_or(0).max(0); }
        _ => {}
    });
    let db = build_workspace_db(&ctx.env, slug).await?;
    let actions = db.list_scheduled_actions(status.as_deref(), limit, offset).await?;
    Response::from_json(&actions)
}

#[derive(Deserialize)]
struct CreateBody {
    action_type: grumps_scheduler::ActionType,
    title: String,
    trigger_at: chrono::DateTime<chrono::Utc>,
    recurrence: Option<String>,
    condition: Option<serde_json::Value>,
    payload: serde_json::Value,
}

pub async fn create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match auth_required(&req, &ctx.env) { Ok(c) => c, Err(r) => return Ok(r) };
    let slug = ctx.param("slug").map(|s| s.as_str()).unwrap_or("");
    if !claims.workspaces.iter().any(|w| w == slug) {
        return respond_error(403, "forbidden", "not a member of this workspace");
    }
    let body: CreateBody = req.json().await
        .map_err(|e| Error::RustError(format!("bad body: {e}")))?;
    let db = build_workspace_db(&ctx.env, slug).await?;
    let new = grumps_scheduler::NewScheduledAction {
        action_type: body.action_type,
        title: body.title,
        trigger_at: body.trigger_at,
        recurrence: body.recurrence,
        condition: body.condition,
        payload: body.payload,
        target_chat: Some("group".into()),
        created_by: Some(claims.sub.clone()),
    };
    // Insert into D1
    let id = db.create_scheduled_action(&new).await?;

    // RPC DO to arm alarm — retry 3x, rollback D1 on final failure
    let rpc_ok = arm_do_alarm(&ctx.env, slug, &new.trigger_at.to_rfc3339()).await;
    if rpc_ok.is_err() {
        let _ = db.delete_scheduled_action(&id).await;
        return respond_error(503, "scheduling_failed", "could not arm scheduler — try again");
    }
    let action = db.get_scheduled_action(&id).await?;
    Response::from_json(&action).map(|r| r.with_status(201))
}

pub async fn get(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match auth_required(&req, &ctx.env) { Ok(c) => c, Err(r) => return Ok(r) };
    let slug = ctx.param("slug").map(|s| s.as_str()).unwrap_or("");
    if !claims.workspaces.iter().any(|w| w == slug) {
        return respond_error(403, "forbidden", "not a member of this workspace");
    }
    let id = ctx.param("id").map(|s| s.as_str()).unwrap_or("");
    let db = build_workspace_db(&ctx.env, slug).await?;
    match db.get_scheduled_action(id).await? {
        Some(a) => Response::from_json(&a),
        None => respond_error(404, "not_found", "scheduled action not found"),
    }
}

pub async fn delete(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match auth_required(&req, &ctx.env) { Ok(c) => c, Err(r) => return Ok(r) };
    let slug = ctx.param("slug").map(|s| s.as_str()).unwrap_or("");
    if !claims.workspaces.iter().any(|w| w == slug) {
        return respond_error(403, "forbidden", "not a member of this workspace");
    }
    let id = ctx.param("id").map(|s| s.as_str()).unwrap_or("");
    let db = build_workspace_db(&ctx.env, slug).await?;
    let deleted = db.delete_scheduled_action(id).await?;
    if !deleted { return respond_error(404, "not_found", "scheduled action not found"); }
    // Tell DO to recompute (since we may have deleted the next-due one)
    let _ = reschedule_do(&ctx.env, slug).await;
    Response::empty().map(|r| r.with_status(204))
}

/// RPC the DO to arm a new alarm.
async fn arm_do_alarm(env: &Env, slug: &str, trigger_at_iso: &str) -> Result<()> {
    let do_ns = env.durable_object("WS_SCHEDULER")?;
    let id = do_ns.id_from_name(slug)?;
    let stub = id.get_stub()?;
    let body = serde_json::json!({ "op": "schedule", "trigger_at": trigger_at_iso });
    let mut headers = Headers::new();
    headers.set("content-type", "application/json")?;
    let req = Request::new_with_init("https://do/", RequestInit::new()
        .with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(serde_json::to_string(&body).unwrap().into())))?;
    // Retry 3x with exponential backoff
    let mut attempts = 0;
    loop {
        attempts += 1;
        match stub.fetch_with_request(req.clone()?).await {
            Ok(resp) if resp.status_code() < 500 => return Ok(()),
            Ok(_) | Err(_) if attempts >= 3 => return Err(Error::RustError("DO RPC failed after 3 retries".into())),
            _ => {
                // Sleep 100ms * 2^attempt — but we can't sleep in workers easily ;
                // just retry immediately (CF DO calls are local-network fast)
            }
        }
    }
}

async fn reschedule_do(env: &Env, slug: &str) -> Result<()> {
    let do_ns = env.durable_object("WS_SCHEDULER")?;
    let id = do_ns.id_from_name(slug)?;
    let stub = id.get_stub()?;
    let body = serde_json::json!({ "op": "reschedule" });
    let mut headers = Headers::new();
    headers.set("content-type", "application/json")?;
    let req = Request::new_with_init("https://do/", RequestInit::new()
        .with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(serde_json::to_string(&body).unwrap().into())))?;
    let _ = stub.fetch_with_request(req).await;
    Ok(())
}
```

- [ ] **Step 2: Add module export**

In `crates/worker/src/routes/mod.rs` :
```rust
pub mod scheduled;
```

- [ ] **Step 3: Wire routes**

Append to `crates/worker/src/lib.rs` `Router::new()` :

```rust
.get_async("/api/w/:slug/scheduled", routes::scheduled::list)
.post_async("/api/w/:slug/scheduled", routes::scheduled::create)
.get_async("/api/w/:slug/scheduled/:id", routes::scheduled::get)
.delete_async("/api/w/:slug/scheduled/:id", routes::scheduled::delete)
```

- [ ] **Step 4: Verify compiles**

Run: `cargo check -p grumps-worker --target wasm32-unknown-unknown`

Expected: zero errors. If `Request` doesn't implement `Clone`, replace the retry loop with a `RequestInit::new()` rebuild each iteration.

- [ ] **Step 5: Commit**

```bash
git add crates/worker/src/routes/scheduled.rs crates/worker/src/routes/mod.rs crates/worker/src/lib.rs
git commit -m "feat(scheduler): REST routes for scheduled_actions with DO RPC + rollback"
```

---

## Task 21 : RAG ingestion — embed + store helpers

**Files:**
- Create: `crates/worker/src/rag.rs`
- Modify: `crates/worker/src/lib.rs`

- [ ] **Step 1: Create the RAG helper module**

Create `crates/worker/src/rag.rs` :

```rust
//! RAG : embed chat messages via Workers AI bge-m3, upsert to Vectorize.
//! See spec § 6.3.

use worker::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct EmbedInput { text: String }

#[derive(Serialize, Deserialize)]
struct EmbedResponse {
    #[serde(default)]
    data: Vec<Vec<f32>>,
    #[serde(default)]
    shape: Vec<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatVectorMetadata {
    pub workspace_slug: String,
    pub platform: String,
    pub sender_member_id: String,
    pub sender_name: String,
    pub text: String,
    pub timestamp: String,
}

pub async fn embed(env: &Env, text: &str) -> Result<Vec<f32>> {
    let ai = env.ai("AI")?;
    let input = serde_json::json!({ "text": text });
    let resp: EmbedResponse = ai.run("@cf/baai/bge-m3", input).await?;
    resp.data.into_iter().next()
        .ok_or_else(|| Error::RustError("empty embeddings response".into()))
}

pub async fn ingest_message(env: &Env, meta: &ChatVectorMetadata) -> Result<()> {
    if meta.text.len() < 20 { return Ok(()); }       // skip too-short messages
    let vector = embed(env, &meta.text).await?;
    let vectorize = env.vectorize("CHAT_RAG")?;
    let id = format!("msg_{}_{}", meta.workspace_slug, uuid::Uuid::new_v4());
    let metadata_json = serde_json::to_value(meta).map_err(|e| Error::RustError(e.to_string()))?;
    let vectors = serde_json::json!([{
        "id": id,
        "values": vector,
        "metadata": metadata_json,
    }]);
    vectorize.upsert(vectors).await?;
    Ok(())
}

pub async fn query_chat_history(env: &Env, workspace_slug: &str, query: &str, limit: u32) -> Result<Vec<QueryHit>> {
    let vector = embed(env, query).await?;
    let vectorize = env.vectorize("CHAT_RAG")?;
    let q = serde_json::json!({
        "vector": vector,
        "topK": limit,
        "filter": { "workspace_slug": { "$eq": workspace_slug } },
        "returnMetadata": true,
    });
    let result: VectorizeQueryResult = vectorize.query(q).await?;
    Ok(result.matches.into_iter().filter_map(|m| {
        m.metadata.and_then(|md| serde_json::from_value::<ChatVectorMetadata>(md).ok())
            .map(|md| QueryHit {
                sender_name: md.sender_name,
                timestamp: md.timestamp,
                text: md.text,
                score: m.score,
            })
    }).collect())
}

#[derive(Deserialize)]
struct VectorizeQueryResult {
    matches: Vec<VectorizeMatch>,
}

#[derive(Deserialize)]
struct VectorizeMatch {
    score: f32,
    #[serde(default)]
    metadata: Option<serde_json::Value>,
}

#[derive(Serialize, Debug, Clone)]
pub struct QueryHit {
    pub sender_name: String,
    pub timestamp: String,
    pub text: String,
    pub score: f32,
}
```

- [ ] **Step 2: Wire module in lib.rs**

In `crates/worker/src/lib.rs`, add `mod rag;` to the module declarations.

- [ ] **Step 3: Verify compiles**

Run: `cargo check -p grumps-worker --target wasm32-unknown-unknown`

> **Note** : If `env.vectorize("CHAT_RAG")?.upsert(...)` or `query(...)` doesn't match the workers-rs 0.8 API exactly, look up the correct method names in the workers-rs docs or `Cargo.lock` for vectorize-related types. The signatures may use `Vec<Vector>` typed builders rather than raw JSON. Adapt as needed.
> If `env.ai("AI")?.run(...)` API differs, same — use the typed equivalent. The structure should remain identical.

Expected after adaptation: zero errors.

- [ ] **Step 4: Commit**

```bash
git add crates/worker/src/rag.rs crates/worker/src/lib.rs
git commit -m "feat(rag): embed + ingest + query helpers via Workers AI bge-m3 + Vectorize"
```

---

## Task 22 : Hook RAG ingestion into webhook handlers

**Files:**
- Modify: `crates/worker/src/routes/webhook.rs` (WhatsApp)
- Modify: `crates/worker/src/routes/webhook_telegram.rs`
- Modify: `crates/worker/src/routes/webhook_discord.rs`

- [ ] **Step 1: Identify the message-receive code path**

Run: `grep -n "InboundMessage\|sender_name\|ws_db" crates/worker/src/routes/webhook_telegram.rs | head -10`

Look for the spot where the inbound message is parsed and the sender member is upserted. The RAG ingest hook goes RIGHT AFTER the member is upserted and BEFORE the message routing.

- [ ] **Step 2: Add RAG ingest call in WhatsApp handler**

In `crates/worker/src/routes/webhook.rs`, find the section after member upsert and before fast-path/agent handling. Insert :

```rust
// RAG ingest (best-effort, non-blocking on failure)
if let Some(text) = msg.text.as_ref() {
    let meta = crate::rag::ChatVectorMetadata {
        workspace_slug: ws.slug.clone(),
        platform: "whatsapp".into(),
        sender_member_id: member_id.clone(),
        sender_name: msg.sender_name.clone(),
        text: text.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    if let Err(e) = crate::rag::ingest_message(&env, &meta).await {
        worker::console_log!("RAG ingest error (whatsapp): {e}");
    }
}
```

> Adapt variable names to match the actual code. The point is : after we have `ws.slug`, `member_id`, `sender_name`, and `text`, call `rag::ingest_message`.

- [ ] **Step 3: Same for Telegram**

In `crates/worker/src/routes/webhook_telegram.rs`, add the same block with `platform: "telegram"`.

- [ ] **Step 4: Same for Discord**

In `crates/worker/src/routes/webhook_discord.rs`, add the same block with `platform: "discord"`.

- [ ] **Step 5: Verify compiles**

Run: `cargo check -p grumps-worker --target wasm32-unknown-unknown`

Expected: zero errors.

- [ ] **Step 6: Commit**

```bash
git add crates/worker/src/routes/webhook.rs crates/worker/src/routes/webhook_telegram.rs crates/worker/src/routes/webhook_discord.rs
git commit -m "feat(rag): hook ingest_message into all 3 webhook handlers"
```

---

## Task 23 : Provisioning extension — apply 0002/0003/0004 on new workspace creation

**Files:**
- Modify: `crates/worker/src/provisioning.rs`

- [ ] **Step 1: Inspect existing provisioning flow**

Run: `grep -n "0001_init\|migrations/workspace" crates/worker/src/provisioning.rs`

Locate where `0001_init.sql` is applied to a new workspace D1.

- [ ] **Step 2: Apply new migrations after 0001**

In `crates/worker/src/provisioning.rs`, find the function that provisions a new workspace D1 (likely `create_workspace_db` or similar). Right after the existing `apply_migration("0001_init.sql")` call, add :

```rust
apply_migration(&client, &database_id, include_str!("../../../migrations/workspace/0002_memory.sql")).await?;
apply_migration(&client, &database_id, include_str!("../../../migrations/workspace/0003_calendar.sql")).await?;
apply_migration(&client, &database_id, include_str!("../../../migrations/workspace/0004_scheduling.sql")).await?;
```

> Adjust the function name `apply_migration` and arguments to match what's actually used. The point is : every new workspace gets all 4 migrations applied.

- [ ] **Step 3: Verify compiles**

Run: `cargo check -p grumps-worker --target wasm32-unknown-unknown`

Expected: zero errors.

- [ ] **Step 4: Commit**

```bash
git add crates/worker/src/provisioning.rs
git commit -m "feat(provisioning): apply migrations 0002/0003/0004 on new workspace"
```

---

## Task 24 : Migration script for existing workspaces

**Files:**
- Create: `scripts/migrate_workspaces.sh`

- [ ] **Step 1: Write the script**

Create `scripts/migrate_workspaces.sh` :

```bash
#!/bin/bash
# Apply migrations 0002/0003/0004 to all existing workspaces.
# Idempotent: safe to re-run.
# See spec § 15.2.

set -euo pipefail

echo "Listing workspaces from Index DB..."
WORKSPACES=$(wrangler d1 execute grumps-index \
    --command "SELECT slug, d1_database_id FROM workspaces_meta" \
    --json --remote \
    | jq -r '.[0].results[] | "\(.d1_database_id)\t\(.slug)"')

if [ -z "$WORKSPACES" ]; then
    echo "No workspaces found. Nothing to migrate."
    exit 0
fi

echo "$WORKSPACES" | while IFS=$'\t' read -r db_id slug; do
    echo ""
    echo "=== Migrating workspace: $slug ($db_id) ==="
    for mig in 0002_memory 0003_calendar 0004_scheduling; do
        echo "  Applying $mig.sql..."
        wrangler d1 execute "$db_id" \
            --file="migrations/workspace/${mig}.sql" \
            --remote \
            || { echo "  FAILED: $slug/$mig — STOPPING"; exit 1; }
    done
    echo "  ✓ Done: $slug"
done

echo ""
echo "All workspaces migrated successfully."
```

- [ ] **Step 2: Make it executable**

Run: `chmod +x scripts/migrate_workspaces.sh`

- [ ] **Step 3: Commit**

```bash
git add scripts/migrate_workspaces.sh
git commit -m "feat(migration): script to apply 0002/0003/0004 to existing workspaces"
```

---

## Task 25 : Reminder migration script (existing reminders → scheduled_actions)

**Files:**
- Create: `migrations/workspace/0005_migrate_reminders.sql`

- [ ] **Step 1: Write the migration**

Create `migrations/workspace/0005_migrate_reminders.sql` :

```sql
-- Migrate existing reminders rows to scheduled_actions for unified handling.
-- See spec § 15.3.
-- Idempotent: NOT EXISTS check prevents double-insert.

INSERT INTO scheduled_actions (id, action_type, title, trigger_at, recurrence, payload, target_chat, created_by)
SELECT id, 'reminder', title, remind_at, recurrence,
       json_object('text', title, 'creator_member_id', created_by),
       'group', created_by
FROM reminders
WHERE status = 'active'
  AND remind_at > datetime('now')
  AND NOT EXISTS (SELECT 1 FROM scheduled_actions sa WHERE sa.id = reminders.id);

-- Keep reminders table intact for historical reference (do NOT drop).
-- The cron-based reminder handler is disabled at next deploy via wrangler.toml.
```

- [ ] **Step 2: Update migrate_workspaces.sh**

Modify `scripts/migrate_workspaces.sh`. In the `for mig in ...` loop, change the list :

```bash
for mig in 0002_memory 0003_calendar 0004_scheduling 0005_migrate_reminders; do
```

- [ ] **Step 3: Commit**

```bash
git add migrations/workspace/0005_migrate_reminders.sql scripts/migrate_workspaces.sh
git commit -m "feat(migration): migrate existing reminders to scheduled_actions"
```

---

## Task 26 : Wrangler — disable old cron, deploy DO migration

**Files:**
- Modify: `wrangler.toml`

- [ ] **Step 1: Remove the old cron schedule**

In `wrangler.toml`, the existing `[triggers] crons = ["*/5 * * * *"]` should be removed (or set to empty array). Recaps now go through DO scheduler, no cron needed.

Change :
```toml
[triggers]
crons = ["*/5 * * * *"]
```
to :
```toml
[triggers]
crons = []
```

(or remove the `[triggers]` block entirely)

- [ ] **Step 2: Verify wrangler.toml**

Run: `wrangler types`

Expected: parses OK.

- [ ] **Step 3: Commit**

```bash
git add wrangler.toml
git commit -m "chore(infra): remove cron — recaps now via DO scheduler"
```

---

## Task 27 : Smoke integration test — memory CRUD via wrangler dev

**Files:**
- Create: `crates/worker/tests/integration_memory.rs`

> **Note** : Integration tests against a real worker require `wrangler dev` running in background. We make these `#[ignore]`-d by default so CI doesn't break. Engineer runs them manually.

- [ ] **Step 1: Write the test scaffold**

Create `crates/worker/tests/integration_memory.rs` :

```rust
//! Integration test for memory CRUD via REST.
//! Requires : `wrangler dev --local` running on http://localhost:8787
//!           a test workspace exists in the local Index DB
//!           a valid JWT for that workspace in env var GRUMPS_TEST_JWT
//!
//! Run with : cargo test --target x86_64-pc-windows-msvc --test integration_memory -- --ignored --nocapture

use serde_json::json;

const BASE: &str = "http://localhost:8787";

fn jwt() -> String {
    std::env::var("GRUMPS_TEST_JWT").expect("set GRUMPS_TEST_JWT")
}

fn slug() -> String {
    std::env::var("GRUMPS_TEST_SLUG").expect("set GRUMPS_TEST_SLUG")
}

#[test]
#[ignore]
fn create_get_update_delete_memory() {
    let client = reqwest::blocking::Client::new();
    let auth = format!("Bearer {}", jwt());

    // 1. Create
    let resp = client.post(format!("{BASE}/api/w/{}/memory", slug()))
        .header("authorization", &auth)
        .json(&json!({
            "value": "wifi du bureau = XYZ123",
            "kind": "fact",
            "pinned": true
        }))
        .send().unwrap();
    assert_eq!(resp.status(), 201);
    let created: serde_json::Value = resp.json().unwrap();
    let id = created["id"].as_str().unwrap().to_string();

    // 2. Get
    let resp = client.get(format!("{BASE}/api/w/{}/memory/{id}", slug()))
        .header("authorization", &auth)
        .send().unwrap();
    assert_eq!(resp.status(), 200);
    let fetched: serde_json::Value = resp.json().unwrap();
    assert_eq!(fetched["value"], "wifi du bureau = XYZ123");
    assert_eq!(fetched["pinned"], true);

    // 3. Update
    let resp = client.put(format!("{BASE}/api/w/{}/memory/{id}", slug()))
        .header("authorization", &auth)
        .json(&json!({ "value": "wifi du bureau = ABC456" }))
        .send().unwrap();
    assert_eq!(resp.status(), 200);
    let updated: serde_json::Value = resp.json().unwrap();
    assert_eq!(updated["value"], "wifi du bureau = ABC456");

    // 4. List should include it
    let resp = client.get(format!("{BASE}/api/w/{}/memory", slug()))
        .header("authorization", &auth)
        .send().unwrap();
    assert_eq!(resp.status(), 200);
    let list: Vec<serde_json::Value> = resp.json().unwrap();
    assert!(list.iter().any(|e| e["id"] == id));

    // 5. Delete
    let resp = client.delete(format!("{BASE}/api/w/{}/memory/{id}", slug()))
        .header("authorization", &auth)
        .send().unwrap();
    assert_eq!(resp.status(), 204);

    // 6. Get should be 404
    let resp = client.get(format!("{BASE}/api/w/{}/memory/{id}", slug()))
        .header("authorization", &auth)
        .send().unwrap();
    assert_eq!(resp.status(), 404);
}
```

- [ ] **Step 2: Add reqwest dev-dependency**

Modify `crates/worker/Cargo.toml`. Add (or extend if `[dev-dependencies]` exists) :

```toml
[dev-dependencies]
reqwest = { version = "0.12", features = ["blocking", "json"] }
```

- [ ] **Step 3: Verify the test compiles (ignored doesn't run by default)**

Run: `cargo test -p grumps-worker --target x86_64-pc-windows-msvc --test integration_memory --no-run`

Expected: compiles cleanly.

To actually run (manually) :
```bash
# Terminal 1
wrangler dev --local

# Terminal 2 (after creating a test workspace + JWT)
GRUMPS_TEST_JWT="..." GRUMPS_TEST_SLUG="..." \
  cargo test --target x86_64-pc-windows-msvc --test integration_memory -- --ignored --nocapture
```

- [ ] **Step 4: Commit**

```bash
git add crates/worker/tests/integration_memory.rs crates/worker/Cargo.toml
git commit -m "test(memory): integration test scaffold for CRUD via REST"
```

---

## Task 28 : Smoke integration test — scheduled action create + DO arm + alarm fire

**Files:**
- Create: `crates/worker/tests/integration_scheduled.rs`

- [ ] **Step 1: Write the test**

Create `crates/worker/tests/integration_scheduled.rs` :

```rust
//! Integration test : create scheduled action with trigger_at = now+90s,
//! wait, verify DO fires it (message arrives in test channel).
//! Requires the same setup as integration_memory.rs.
//! Run : cargo test --target x86_64-pc-windows-msvc --test integration_scheduled -- --ignored --nocapture

use serde_json::json;
use std::thread;
use std::time::Duration;

const BASE: &str = "http://localhost:8787";

fn jwt() -> String { std::env::var("GRUMPS_TEST_JWT").expect("set GRUMPS_TEST_JWT") }
fn slug() -> String { std::env::var("GRUMPS_TEST_SLUG").expect("set GRUMPS_TEST_SLUG") }

#[test]
#[ignore]
fn create_reminder_fires_via_do_alarm() {
    let client = reqwest::blocking::Client::new();
    let auth = format!("Bearer {}", jwt());

    // Trigger 90s in future — short enough for test, long enough for DO arming
    let trigger = chrono::Utc::now() + chrono::Duration::seconds(90);
    let resp = client.post(format!("{BASE}/api/w/{}/scheduled", slug()))
        .header("authorization", &auth)
        .json(&json!({
            "action_type": "reminder",
            "title": "Test reminder",
            "trigger_at": trigger.to_rfc3339(),
            "payload": { "text": "Test reminder fired!" }
        }))
        .send().unwrap();
    assert_eq!(resp.status(), 201);
    let created: serde_json::Value = resp.json().unwrap();
    let id = created["id"].as_str().unwrap().to_string();

    println!("Action created: {id}, waiting 100s for DO alarm...");
    thread::sleep(Duration::from_secs(100));

    // Status should now be 'done'
    let resp = client.get(format!("{BASE}/api/w/{}/scheduled/{id}", slug()))
        .header("authorization", &auth)
        .send().unwrap();
    assert_eq!(resp.status(), 200);
    let action: serde_json::Value = resp.json().unwrap();
    assert_eq!(action["status"], "done", "action did not fire (status: {:?})", action["status"]);
    assert!(action["last_fired_at"].is_string());

    // Manually verify in the test chat that "Test reminder fired!" was received.
    println!("✓ Action marked done. Verify message in test chat manually.");
}
```

- [ ] **Step 2: Compile-check**

Run: `cargo test -p grumps-worker --target x86_64-pc-windows-msvc --test integration_scheduled --no-run`

Expected: compiles.

- [ ] **Step 3: Commit**

```bash
git add crates/worker/tests/integration_scheduled.rs
git commit -m "test(scheduler): integration test for scheduled action + DO alarm fire"
```

---

## Task 29 : Final compile and full test suite

**Files:** none new

- [ ] **Step 1: Full workspace compile (native)**

Run: `cargo check --workspace --target x86_64-pc-windows-msvc`

Expected: zero errors.

- [ ] **Step 2: Full workspace compile (wasm32 worker)**

Run: `cargo check -p grumps-worker --target wasm32-unknown-unknown`

Expected: zero errors.

- [ ] **Step 3: Run all unit tests**

Run: `cargo test --workspace --target x86_64-pc-windows-msvc --exclude grumps-spa`

(Excluding spa as it's CSR/wasm — not affected by this plan.)

Expected:
- grumps-memory: 3 passed
- grumps-calendar: 2 passed
- grumps-scheduler: 2 (action) + 9 (condition) + 11 (recurrence) + 2 (session) = 24 passed
- grumps-worker (lib tests, if any) : passes

- [ ] **Step 4: Build the worker for deploy**

Run (from project root) :
```bash
cd crates/worker && worker-build --release && cd ../..
```

Expected: `Built worker successfully` or equivalent. No errors.

- [ ] **Step 5: Tag the milestone**

```bash
git tag plan-A-foundation
git log --oneline -30   # verify the chain of commits
```

---

## Self-Review du plan

### Couverture du spec
- ✅ Data model (memory_entries, events, scheduled_actions, agent_sessions, settings extensions) — Tasks 2, 3, 4
- ✅ Memory layer CRUD + FTS — Tasks 5, 6
- ✅ Calendar events table + CRUD — Tasks 7, 8
- ✅ Scheduler types + CRUD — Tasks 9, 13
- ✅ Condition evaluator (5 types) — Task 10
- ✅ RRULE parser + next_occurrence — Task 11
- ✅ Agent sessions types + CRUD — Tasks 12, 13
- ✅ DO `WorkspaceScheduler` — Tasks 15, 17
- ✅ Executor (reminder/event_notify/recap) — Task 16
- ✅ REST routes (memory/events/scheduled) — Tasks 18, 19, 20
- ✅ DO RPC + retry/rollback — Task 20
- ✅ RAG ingestion — Tasks 21, 22
- ✅ Provisioning extension — Task 23
- ✅ Migration scripts (existing workspaces + reminders) — Tasks 24, 25
- ✅ Removal of old cron — Task 26
- ✅ Integration tests scaffold — Tasks 27, 28

### Hors scope (Plans suivants)
- Plan B : agent loop, tool dispatch, cascade routing (Gemini classifier), prompt builder, Sonnet integration
- Plan C : calendar aggregation view, iCal export, full UI
- Plan D : web search provider abstraction
- Plan E : auto-extract pipeline, proactive mode, consent flows
- Plan F : SPA pages (memory/scheduled/calendar)
- Plan G : deployment runbook, RAG backfill, monitoring

### Open questions traitées
- ✅ § 19.1 RRULE crate compatibility → Plan A implements manuel (Task 11) — ne dépend d'aucune crate externe
- ⚠️ § 19.2 bge-m3 disponibilité Workers AI → vérifier au moment du Step 3 de Task 21. Si pas dispo, fallback `@cf/baai/bge-base-en-v1.5` (768-dim au lieu de 1024 — change le `dimensions=1024` dans `wrangler vectorize create`)
- ⚠️ § 19.3 DO + alarm stability workers-rs 0.8 → assumé stable. Si bug, fallback : Cron Trigger 1×/min comme dans la version "polling" rejetée § 7.1 spec
- ✅ § 19.4 prompt cache Anthropic → traité en Plan B

### Plan B preview (préparation mentale)
Une fois Plan A déployé et fonctionnel, Plan B ajoutera :
- Crate `grumps-agent` avec `loop.rs`, `prompt.rs`, `session.rs`, `router.rs`, `tools/`
- Sonnet client wrapper (HTTP via Fetch)
- Gemini classifier pour cascade routing
- 11 tools wrappés avec leurs JSON schemas
- Wire dans `handler.rs` après le fast-path existant
- Executor `agent_task` + `follow_up` (qui sont stubbed dans Plan A)

---

*Fin du Plan A. ~29 tâches, ~150 steps, estimation 3-5 jours d'implémentation par 1 développeur expérimenté Rust + Workers.*
