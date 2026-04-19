// crates/worker/src/db.rs
use worker::*;
use crate::d1_rest::{D1RestClient, D1Response, extract_first, extract_rows};
use serde::{Deserialize, Serialize};
use grumps_memory::{MemoryEntry, MemoryKind, MemorySource, NewMemoryEntry};

// =============================================
// Index DB (native binding)
// =============================================

pub fn get_index_db(env: &Env) -> Result<D1Database> {
    env.d1("INDEX_DB")
}

#[derive(Deserialize, Debug, Clone)]
pub struct WorkspaceMetaRow {
    pub slug: String,
    pub d1_database_id: String,
    pub name: Option<String>,
    pub plan: String,
}

pub async fn lookup_workspace_by_slug(index_db: &D1Database, slug: &str) -> Result<Option<WorkspaceMetaRow>> {
    index_db.prepare("SELECT slug, d1_database_id, name, plan FROM workspaces_meta WHERE slug = ?1")
        .bind(&[slug.into()])?.first::<WorkspaceMetaRow>(None).await
}

pub async fn lookup_workspace(index_db: &D1Database, platform: &str, channel_id: &str) -> Result<Option<WorkspaceMetaRow>> {
    index_db.prepare("SELECT slug, d1_database_id, name, plan FROM workspaces_meta WHERE platform = ?1 AND platform_channel_id = ?2")
        .bind(&[platform.into(), channel_id.into()])?.first::<WorkspaceMetaRow>(None).await
}

/// Upsert a user in the Index DB and link them to a workspace.
pub async fn upsert_index_user(index_db: &D1Database, phone: &str, workspace_slug: &str, role: &str) -> Result<()> {
    let user_id = uuid::Uuid::new_v4().to_string();
    // Upsert user
    index_db.prepare("INSERT INTO users (id, phone) VALUES (?1, ?2) ON CONFLICT(phone) DO NOTHING")
        .bind(&[user_id.clone().into(), phone.into()])?.run().await?;
    // Get actual user id
    #[derive(Deserialize)]
    struct Row { id: String }
    let row = index_db.prepare("SELECT id FROM users WHERE phone = ?1")
        .bind(&[phone.into()])?.first::<Row>(None).await?;
    let uid = row.map(|r| r.id).unwrap_or(user_id);
    // Link to workspace
    index_db.prepare("INSERT INTO user_workspaces (user_id, workspace_slug, role) VALUES (?1, ?2, ?3) ON CONFLICT(user_id, workspace_slug) DO NOTHING")
        .bind(&[uid.into(), workspace_slug.into(), role.into()])?.run().await?;
    Ok(())
}

// =============================================
// Workspace DB (via D1 REST API)
// =============================================

pub struct WorkspaceDb<'a> {
    client: &'a D1RestClient,
    database_id: String,
}

impl<'a> WorkspaceDb<'a> {
    pub fn new(client: &'a D1RestClient, database_id: String) -> Self {
        Self { client, database_id }
    }

    async fn q(&self, sql: &str, params: Vec<serde_json::Value>) -> Result<D1Response> {
        self.client.query(&self.database_id, sql, params).await
    }

    // --- Members ---

    /// Upsert member. Returns (member_id, is_first_member).
    /// First member becomes admin automatically.
    pub async fn upsert_member(&self, platform_user_id: &str, display_name: &str) -> Result<(String, bool)> {
        // Check if any members exist
        #[derive(Deserialize)]
        struct CountRow { cnt: i64 }
        let resp = self.q("SELECT COUNT(*) as cnt FROM members", vec![]).await?;
        let count: Option<CountRow> = extract_first(&resp)?;
        let is_first = count.map(|c| c.cnt == 0).unwrap_or(true);
        let role = if is_first { "admin" } else { "member" };

        let id = uuid::Uuid::new_v4().to_string();
        self.q(
            "INSERT INTO members (id, platform_user_id, display_name, role, last_seen_at) VALUES (?1, ?2, ?3, ?4, datetime('now')) ON CONFLICT(platform_user_id) DO UPDATE SET display_name = ?3, last_seen_at = datetime('now')",
            vec![id.clone().into(), platform_user_id.into(), display_name.into(), role.into()],
        ).await?;

        #[derive(Deserialize)]
        struct IdRow { id: String }
        let resp = self.q("SELECT id FROM members WHERE platform_user_id = ?1", vec![platform_user_id.into()]).await?;
        let row: Option<IdRow> = extract_first(&resp)?;
        Ok((row.map(|r| r.id).unwrap_or(id), is_first))
    }

    // --- Todos ---

    /// Insert todo with atomic seq_num. Returns (todo_id, seq_num).
    pub async fn insert_todo(&self, title: &str, priority: i32, tags_json: &str,
                              assigned_to: &str, assigned_name: &str,
                              created_by: &str, source: &str, message_id: &str) -> Result<(String, i64)> {
        let id = uuid::Uuid::new_v4().to_string();
        self.q(
            "INSERT INTO todos (id, seq_num, title, status, priority, tags, assigned_to, assigned_name, created_by, source, message_id, created_at, updated_at) VALUES (?1, (SELECT COALESCE(MAX(seq_num), 0) + 1 FROM todos), ?2, 'open', ?3, ?4, NULLIF(?5,''), NULLIF(?6,''), ?7, ?8, NULLIF(?9,''), datetime('now'), datetime('now'))",
            vec![id.clone().into(), title.into(), priority.into(), tags_json.into(), assigned_to.into(), assigned_name.into(), created_by.into(), source.into(), message_id.into()],
        ).await?;

        #[derive(Deserialize)]
        struct SeqRow { seq_num: i64 }
        let resp = self.q("SELECT seq_num FROM todos WHERE id = ?1", vec![id.clone().into()]).await?;
        let row: Option<SeqRow> = extract_first(&resp)?;
        Ok((id, row.map(|r| r.seq_num).unwrap_or(1)))
    }

    /// Get open todos for fuzzy matching. Returns (id, title, seq_num).
    pub async fn get_open_todos(&self) -> Result<Vec<(String, String, i64)>> {
        #[derive(Deserialize)]
        struct Row { id: String, title: String, seq_num: i64 }
        let resp = self.q("SELECT id, title, seq_num FROM todos WHERE status IN ('open', 'in_progress')", vec![]).await?;
        let rows: Vec<Row> = extract_rows(&resp)?;
        Ok(rows.into_iter().map(|r| (r.id, r.title, r.seq_num)).collect())
    }

    /// Get todos with filter. Returns (seq_num, title, status, assignee_name, priority, tags).
    pub async fn get_todos_filtered(&self, filter: &str, member_id: Option<&str>) -> Result<Vec<(i64, String, String, Option<String>, i32, String)>> {
        #[derive(Deserialize)]
        struct Row { seq_num: i64, title: String, status: String, assigned_name: Option<String>, priority: i32, tags: String }

        let (sql, params): (&str, Vec<serde_json::Value>) = match filter {
            "open" => ("SELECT seq_num, title, status, assigned_name, priority, tags FROM todos WHERE status IN ('open','in_progress') ORDER BY priority ASC, created_at DESC", vec![]),
            "all" => ("SELECT seq_num, title, status, assigned_name, priority, tags FROM todos WHERE status != 'deleted' ORDER BY created_at DESC", vec![]),
            "done" => ("SELECT seq_num, title, status, assigned_name, priority, tags FROM todos WHERE status = 'done' ORDER BY completed_at DESC", vec![]),
            "mine" => ("SELECT seq_num, title, status, assigned_name, priority, tags FROM todos WHERE assigned_to = ?1 AND status IN ('open','in_progress') ORDER BY priority ASC", vec![member_id.unwrap_or("").into()]),
            _ if filter.starts_with("assignee:") => ("SELECT seq_num, title, status, assigned_name, priority, tags FROM todos WHERE assigned_name = ?1 AND status IN ('open','in_progress') ORDER BY priority ASC", vec![filter[9..].into()]),
            _ if filter.starts_with("tag:") => ("SELECT seq_num, title, status, assigned_name, priority, tags FROM todos WHERE tags LIKE ?1 AND status IN ('open','in_progress') ORDER BY priority ASC", vec![format!("%\"{}\"%" , &filter[4..]).into()]),
            _ => ("SELECT seq_num, title, status, assigned_name, priority, tags FROM todos WHERE status IN ('open','in_progress') ORDER BY priority ASC", vec![]),
        };

        let resp = self.q(sql, params).await?;
        let rows: Vec<Row> = extract_rows(&resp)?;
        Ok(rows.into_iter().map(|r| (r.seq_num, r.title, r.status, r.assigned_name, r.priority, r.tags)).collect())
    }

    /// Get todo by sequence number.
    pub async fn get_todo_by_seq(&self, seq_num: i64) -> Result<Option<TodoRow>> {
        let resp = self.q("SELECT id, seq_num, title, status, recurrence FROM todos WHERE seq_num = ?1", vec![seq_num.into()]).await?;
        extract_first(&resp)
    }

    /// Get todo by id.
    pub async fn get_todo_by_id(&self, todo_id: &str) -> Result<Option<TodoRow>> {
        let resp = self.q("SELECT id, seq_num, title, status, recurrence FROM todos WHERE id = ?1", vec![todo_id.into()]).await?;
        extract_first(&resp)
    }

    pub async fn complete_todo(&self, todo_id: &str, completed_by: &str) -> Result<()> {
        self.q("UPDATE todos SET status = 'done', completed_at = datetime('now'), completed_by = ?1, updated_at = datetime('now') WHERE id = ?2",
            vec![completed_by.into(), todo_id.into()]).await?;
        Ok(())
    }

    pub async fn delete_todo(&self, todo_id: &str) -> Result<()> {
        self.q("UPDATE todos SET status = 'deleted', updated_at = datetime('now') WHERE id = ?1", vec![todo_id.into()]).await?;
        Ok(())
    }

    // --- Bot message tracking ---

    pub async fn track_bot_message(&self, message_id: &str, todo_id: Option<&str>) -> Result<()> {
        self.q("INSERT OR IGNORE INTO bot_messages (message_id, todo_id) VALUES (?1, NULLIF(?2,''))",
            vec![message_id.into(), todo_id.unwrap_or("").into()]).await?;
        Ok(())
    }

    pub async fn is_bot_message(&self, message_id: &str) -> Result<bool> {
        #[derive(Deserialize)]
        struct Row { message_id: String }
        let resp = self.q("SELECT message_id FROM bot_messages WHERE message_id = ?1", vec![message_id.into()]).await?;
        let row: Option<Row> = extract_first(&resp)?;
        Ok(row.is_some())
    }

    pub async fn get_todo_for_bot_message(&self, message_id: &str) -> Result<Option<String>> {
        #[derive(Deserialize)]
        struct Row { todo_id: Option<String> }
        let resp = self.q("SELECT todo_id FROM bot_messages WHERE message_id = ?1", vec![message_id.into()]).await?;
        let row: Option<Row> = extract_first(&resp)?;
        Ok(row.and_then(|r| r.todo_id))
    }

    // --- Notes ---

    pub async fn insert_note(&self, title: &str, content: &str, source: &str, created_by: &str) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        self.q("INSERT INTO notes (id, title, content, source, created_by, created_at, updated_at) VALUES (?1, NULLIF(?2,''), ?3, ?4, ?5, datetime('now'), datetime('now'))",
            vec![id.clone().into(), title.into(), content.into(), source.into(), created_by.into()]).await?;
        Ok(id)
    }

    /// Get all notes. Returns (id, title, source, created_at).
    pub async fn get_notes(&self) -> Result<Vec<(String, Option<String>, String, String)>> {
        #[derive(Deserialize)]
        struct Row { id: String, title: Option<String>, source: String, created_at: String }
        let resp = self.q("SELECT id, title, source, created_at FROM notes ORDER BY created_at DESC", vec![]).await?;
        let rows: Vec<Row> = extract_rows(&resp)?;
        Ok(rows.into_iter().map(|r| (r.id, r.title, r.source, r.created_at)).collect())
    }

    /// Search notes (basic LIKE search — FTS5 requires native binding).
    pub async fn search_notes(&self, query: &str) -> Result<Vec<(String, Option<String>, String, String)>> {
        #[derive(Deserialize)]
        struct Row { id: String, title: Option<String>, source: String, created_at: String }
        let pattern = format!("%{}%", query);
        let resp = self.q("SELECT id, title, source, created_at FROM notes WHERE title LIKE ?1 OR content LIKE ?1 ORDER BY created_at DESC",
            vec![pattern.into()]).await?;
        let rows: Vec<Row> = extract_rows(&resp)?;
        Ok(rows.into_iter().map(|r| (r.id, r.title, r.source, r.created_at)).collect())
    }

    // --- Activity log ---

    pub async fn log_activity(&self, actor: &str, action: &str, target_type: &str, target_id: &str, source: &str) -> Result<()> {
        let id = uuid::Uuid::new_v4().to_string();
        self.q("INSERT INTO activity_log (id, actor, action, target_type, target_id, source, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
            vec![id.into(), actor.into(), action.into(), target_type.into(), target_id.into(), source.into()]).await?;
        Ok(())
    }

    // --- Notes (extended) ---

    pub async fn get_note_by_id(&self, note_id: &str) -> Result<Option<NoteRow>> {
        let resp = self.q("SELECT id, title, content, COALESCE(pinned, 0) as pinned, source, created_by, created_at, updated_at FROM notes WHERE id = ?1", vec![note_id.into()]).await?;
        extract_first(&resp)
    }

    pub async fn update_note(&self, note_id: &str, title: &str, content: &str, _editor_id: &str) -> Result<()> {
        self.q("UPDATE notes SET title = NULLIF(?1,''), content = ?2, updated_at = datetime('now') WHERE id = ?3",
            vec![title.into(), content.into(), note_id.into()]).await?;
        Ok(())
    }

    pub async fn delete_note(&self, note_id: &str) -> Result<()> {
        self.q("DELETE FROM notes WHERE id = ?1", vec![note_id.into()]).await?;
        Ok(())
    }

    // --- Todos (extended) ---

    pub async fn update_todo(&self, todo_id: &str, title: Option<&str>, status: Option<&str>, priority: Option<i32>, assigned_to: Option<&str>, assigned_name: Option<&str>) -> Result<()> {
        // Build dynamic update — use COALESCE to keep existing values when None passed as empty string sentinel
        let title_val = title.unwrap_or("");
        let status_val = status.unwrap_or("");
        let priority_val = priority.unwrap_or(-1);
        let assigned_to_val = assigned_to.unwrap_or("");
        let assigned_name_val = assigned_name.unwrap_or("");
        self.q(
            "UPDATE todos SET \
             title = CASE WHEN ?2 != '' THEN ?2 ELSE title END, \
             status = CASE WHEN ?3 != '' THEN ?3 ELSE status END, \
             priority = CASE WHEN ?4 >= 0 THEN ?4 ELSE priority END, \
             assigned_to = CASE WHEN ?5 != '' THEN NULLIF(?5,'') ELSE assigned_to END, \
             assigned_name = CASE WHEN ?6 != '' THEN NULLIF(?6,'') ELSE assigned_name END, \
             updated_at = datetime('now') WHERE id = ?1",
            vec![todo_id.into(), title_val.into(), status_val.into(), priority_val.into(), assigned_to_val.into(), assigned_name_val.into()],
        ).await?;
        Ok(())
    }

    // --- Activity log (extended) ---

    pub async fn get_activity_log(&self, limit: i64) -> Result<Vec<ActivityRow>> {
        let resp = self.q("SELECT id, actor, action, target_type, target_id, source, created_at FROM activity_log ORDER BY created_at DESC LIMIT ?1", vec![limit.into()]).await?;
        extract_rows(&resp)
    }

    // --- Members (read) ---

    pub async fn get_members(&self) -> Result<Vec<MemberRow>> {
        let resp = self.q("SELECT id, platform_user_id, display_name, role FROM members ORDER BY role ASC, display_name ASC", vec![]).await?;
        extract_rows(&resp)
    }

    // --- Reminders ---

    /// Insert a reminder.
    pub async fn insert_reminder(&self, title: &str, remind_at: &str, recurrence: Option<&str>, target_member: &str, created_by: &str) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        self.q(
            "INSERT INTO reminders (id, title, remind_at, recurrence, target_member, status, created_by, created_at) VALUES (?1, ?2, ?3, ?4, ?5, 'active', ?6, datetime('now'))",
            vec![id.clone().into(), title.into(), remind_at.into(), recurrence.unwrap_or("").into(), target_member.into(), created_by.into()],
        ).await?;
        Ok(id)
    }

    /// Get active reminders that should fire now (remind_at <= now).
    pub async fn get_due_reminders(&self) -> Result<Vec<ReminderRow>> {
        let resp = self.q(
            "SELECT id, title, remind_at, recurrence, target_member, created_by FROM reminders WHERE status = 'active' AND remind_at <= datetime('now')",
            vec![],
        ).await?;
        extract_rows(&resp)
    }

    /// Mark a reminder as fired. If recurring, update remind_at to next occurrence.
    pub async fn fire_reminder(&self, reminder_id: &str, recurrence: Option<&str>) -> Result<()> {
        if let Some(rec) = recurrence {
            if !rec.is_empty() {
                let interval = if rec.contains("daily") || rec.contains("every day") {
                    "+1 day"
                } else if rec.contains("weekly") || rec.contains("every week") || rec.contains("every monday") || rec.contains("every tuesday") || rec.contains("every wednesday") || rec.contains("every thursday") || rec.contains("every friday") || rec.contains("every saturday") || rec.contains("every sunday") {
                    "+7 days"
                } else {
                    return self.q("UPDATE reminders SET status = 'fired' WHERE id = ?1", vec![reminder_id.into()]).await.map(|_| ());
                };
                self.q(
                    &format!("UPDATE reminders SET remind_at = datetime(remind_at, '{}') WHERE id = ?1", interval),
                    vec![reminder_id.into()],
                ).await?;
                return Ok(());
            }
        }
        self.q("UPDATE reminders SET status = 'fired' WHERE id = ?1", vec![reminder_id.into()]).await?;
        Ok(())
    }

    /// List active reminders.
    pub async fn get_active_reminders(&self) -> Result<Vec<ReminderRow>> {
        let resp = self.q("SELECT id, title, remind_at, recurrence, target_member, created_by FROM reminders WHERE status = 'active' ORDER BY remind_at ASC", vec![]).await?;
        extract_rows(&resp)
    }

    /// Cancel a reminder.
    pub async fn cancel_reminder(&self, reminder_id: &str) -> Result<()> {
        self.q("UPDATE reminders SET status = 'cancelled' WHERE id = ?1", vec![reminder_id.into()]).await?;
        Ok(())
    }

    // --- LLM call tracking ---

    /// Increment LLM call counter for this month. Uses settings table.
    pub async fn increment_llm_calls(&self) -> Result<i64> {
        let month_key = {
            use std::time::{SystemTime, UNIX_EPOCH};
            let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
            // Approximate year/month from epoch seconds (good enough for monthly bucketing)
            let days = secs / 86400;
            let year = 1970 + days / 365;
            let day_of_year = days % 365;
            let month = day_of_year / 30 + 1;
            format!("llm_calls_{}_{:02}", year, month.min(12))
        };

        // Upsert counter
        self.q(
            "INSERT INTO settings (key, value) VALUES (?1, '1') ON CONFLICT(key) DO UPDATE SET value = CAST(CAST(value AS INTEGER) + 1 AS TEXT)",
            vec![month_key.clone().into()],
        ).await?;

        // Get current count
        #[derive(serde::Deserialize)]
        struct Row { value: String }
        let resp = self.q("SELECT value FROM settings WHERE key = ?1", vec![month_key.into()]).await?;
        let row: Option<Row> = extract_first(&resp)?;
        Ok(row.and_then(|r| r.value.parse().ok()).unwrap_or(1))
    }

    /// Get current LLM call count for this month.
    pub async fn get_llm_calls_this_month(&self) -> Result<i64> {
        let month_key = {
            use std::time::{SystemTime, UNIX_EPOCH};
            let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
            let days = secs / 86400;
            let year = 1970 + days / 365;
            let day_of_year = days % 365;
            let month = day_of_year / 30 + 1;
            format!("llm_calls_{}_{:02}", year, month.min(12))
        };
        #[derive(serde::Deserialize)]
        struct Row { value: String }
        let resp = self.q("SELECT value FROM settings WHERE key = ?1", vec![month_key.into()]).await?;
        let row: Option<Row> = extract_first(&resp)?;
        Ok(row.and_then(|r| r.value.parse().ok()).unwrap_or(0))
    }

    // --- Settings ---

    /// Get a setting value by key, returns None if missing.
    pub async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        #[derive(Deserialize)]
        struct Row { value: String }
        let resp = self.q("SELECT value FROM settings WHERE key = ?1", vec![key.into()]).await?;
        let row: Option<Row> = extract_first(&resp)?;
        Ok(row.map(|r| r.value))
    }

    // --- Recap ---

    /// Get data needed for a recap.
    pub async fn get_recap_data(&self) -> Result<RecapData> {
        #[derive(Deserialize)]
        struct Count { cnt: i64 }

        // Open todos
        let r = self.q("SELECT COUNT(*) as cnt FROM todos WHERE status IN ('open','in_progress')", vec![]).await?;
        let open: i64 = extract_first::<Count>(&r)?.map(|c| c.cnt).unwrap_or(0);

        // Assigned open todos
        let r = self.q("SELECT COUNT(*) as cnt FROM todos WHERE status IN ('open','in_progress') AND assigned_to IS NOT NULL AND assigned_to != ''", vec![]).await?;
        let assigned: i64 = extract_first::<Count>(&r)?.map(|c| c.cnt).unwrap_or(0);

        // Completed this week
        let r = self.q("SELECT COUNT(*) as cnt FROM todos WHERE status = 'done' AND completed_at >= datetime('now', '-7 days')", vec![]).await?;
        let done_week: i64 = extract_first::<Count>(&r)?.map(|c| c.cnt).unwrap_or(0);

        // High priority open
        let r = self.q("SELECT seq_num, title, assigned_name, deadline FROM todos WHERE status IN ('open','in_progress') AND priority = 1 ORDER BY created_at ASC", vec![]).await?;
        let high_priority: Vec<HighPrioTodo> = extract_rows(&r)?;

        // New notes this week
        let r = self.q("SELECT COUNT(*) as cnt FROM notes WHERE created_at >= datetime('now', '-7 days')", vec![]).await?;
        let new_notes: i64 = extract_first::<Count>(&r)?.map(|c| c.cnt).unwrap_or(0);

        // Active reminders
        let r = self.q("SELECT COUNT(*) as cnt FROM reminders WHERE status = 'active'", vec![]).await?;
        let reminders: i64 = extract_first::<Count>(&r)?.map(|c| c.cnt).unwrap_or(0);

        Ok(RecapData { open, assigned, done_week, high_priority, new_notes, reminders })
    }

    /// Create the next occurrence of a recurring todo when the current one is completed.
    pub async fn create_next_recurrence(&self, todo: &TodoRow, recurrence: &str) -> Result<()> {
        let id = uuid::Uuid::new_v4().to_string();
        self.q(
            "INSERT INTO todos (id, seq_num, title, status, priority, tags, assigned_to, assigned_name, created_by, source, recurrence, created_at, updated_at) VALUES (?1, (SELECT COALESCE(MAX(seq_num), 0) + 1 FROM todos), (SELECT title FROM todos WHERE id = ?2), 'open', (SELECT priority FROM todos WHERE id = ?2), (SELECT tags FROM todos WHERE id = ?2), (SELECT assigned_to FROM todos WHERE id = ?2), (SELECT assigned_name FROM todos WHERE id = ?2), (SELECT created_by FROM todos WHERE id = ?2), 'system', ?3, datetime('now'), datetime('now'))",
            vec![id.into(), todo.id.clone().into(), recurrence.into()],
        ).await?;
        Ok(())
    }

    /// Get recurrence string for a todo.
    pub async fn get_todo_recurrence(&self, todo_id: &str) -> Result<Option<String>> {
        #[derive(serde::Deserialize)]
        struct Row { recurrence: Option<String> }
        let resp = self.q("SELECT recurrence FROM todos WHERE id = ?1", vec![todo_id.into()]).await?;
        let row: Option<Row> = extract_first(&resp)?;
        Ok(row.and_then(|r| r.recurrence).filter(|s| !s.is_empty()))
    }

    // --- Status counts ---

    pub async fn get_status_counts(&self) -> Result<(i64, i64, i64, i64)> {
        #[derive(Deserialize)]
        struct Row { cnt: i64 }

        let r1 = self.q("SELECT COUNT(*) as cnt FROM todos WHERE status IN ('open','in_progress')", vec![]).await?;
        let open: i64 = extract_first::<Row>(&r1)?.map(|r| r.cnt).unwrap_or(0);

        let r2 = self.q("SELECT COUNT(*) as cnt FROM todos WHERE status = 'done' AND completed_at >= datetime('now', '-7 days')", vec![]).await?;
        let done_week: i64 = extract_first::<Row>(&r2)?.map(|r| r.cnt).unwrap_or(0);

        let r3 = self.q("SELECT COUNT(*) as cnt FROM notes", vec![]).await?;
        let notes: i64 = extract_first::<Row>(&r3)?.map(|r| r.cnt).unwrap_or(0);

        // Files table doesn't exist yet (Phase 2), return 0
        Ok((open, done_week, notes, 0))
    }
}

#[derive(Deserialize, Debug)]
pub struct TodoRow {
    pub id: String,
    pub seq_num: i64,
    pub title: String,
    pub status: String,
    pub recurrence: Option<String>,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct NoteRow {
    pub id: String,
    pub title: Option<String>,
    pub content: String,
    pub pinned: i32,
    pub source: String,
    pub created_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct ActivityRow {
    pub id: String,
    pub actor: Option<String>,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub source: String,
    pub created_at: String,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct ReminderRow {
    pub id: String,
    pub title: String,
    pub remind_at: String,
    pub recurrence: Option<String>,
    pub target_member: Option<String>,
    pub created_by: Option<String>,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct MemberRow {
    pub id: String,
    pub platform_user_id: String,
    pub display_name: Option<String>,
    pub role: String,
}

#[derive(Debug)]
pub struct RecapData {
    pub open: i64,
    pub assigned: i64,
    pub done_week: i64,
    pub high_priority: Vec<HighPrioTodo>,
    pub new_notes: i64,
    pub reminders: i64,
}

#[derive(Debug, Deserialize)]
pub struct HighPrioTodo {
    pub seq_num: i64,
    pub title: String,
    pub assigned_name: Option<String>,
    pub deadline: Option<String>,
}

// =============================================
// Memory CRUD (see spec § 6.1)
// =============================================

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
        Ok(resp.result.first().and_then(|rs| rs.meta.as_ref()).and_then(|m| m.changes).unwrap_or(0) > 0)
    }

    pub async fn delete_memory(&self, id: &str) -> Result<bool> {
        let resp = self.q("DELETE FROM memory_entries WHERE id = ?1", vec![id.into()]).await?;
        Ok(resp.result.first().and_then(|rs| rs.meta.as_ref()).and_then(|m| m.changes).unwrap_or(0) > 0)
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
