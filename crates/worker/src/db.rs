// crates/worker/src/db.rs
use worker::*;
use crate::d1_rest::{D1RestClient, D1Response, extract_first, extract_rows};
use serde::Deserialize;

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
        let resp = self.q("SELECT id, seq_num, title, status FROM todos WHERE seq_num = ?1", vec![seq_num.into()]).await?;
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
}
