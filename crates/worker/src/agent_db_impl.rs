//! impl AgentDb for WorkspaceDb.
//! Bridges the agent's database trait to worker's existing WorkspaceDb methods.

use grumps_agent::db::{AgentDb, MemberShort};
use grumps_memory::{MemoryEntry, NewMemoryEntry};
use grumps_calendar::{Event, NewEvent};
use grumps_scheduler::{NewScheduledAction, ScheduledAction, AgentSession, SessionMessage};
use worker::Result;

use crate::db::WorkspaceDb;

#[async_trait::async_trait(?Send)]
impl AgentDb for WorkspaceDb<'_> {
    async fn list_pinned_memory(&self) -> Result<Vec<MemoryEntry>> {
        self.list_pinned_memory().await
    }

    async fn search_memory_fts(&self, query: &str, limit: i64) -> Result<Vec<MemoryEntry>> {
        self.search_memory_fts(query, limit).await
    }

    async fn create_memory(&self, entry: &NewMemoryEntry) -> Result<String> {
        self.create_memory(entry).await
    }

    async fn list_active_members(&self) -> Result<Vec<MemberShort>> {
        let rows = self.get_members().await?;
        Ok(rows.into_iter().map(|r| MemberShort {
            id: r.id,
            display_name: r.display_name,
            role: r.role,
        }).collect())
    }

    async fn create_todo_simple(
        &self,
        title: &str,
        assignee_name: Option<&str>,
        deadline: Option<&str>,
        priority: i32,
        tags: Vec<String>,
        created_by: Option<&str>,
    ) -> Result<String> {
        let tags_json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".into());
        // For deadline, we store it in the todo. WorkspaceDb::insert_todo doesn't take deadline
        // directly — we insert then update the deadline column if present.
        let (id, _seq) = self.insert_todo(
            title,
            priority,
            &tags_json,
            "", // assigned_to (member id — we only have name here, leave blank)
            assignee_name.unwrap_or(""),
            created_by.unwrap_or(""),
            "agent",
            "",
        ).await?;

        // Store deadline if provided.
        if let Some(dl) = deadline {
            self.set_todo_deadline(&id, dl).await?;
        }

        Ok(id)
    }

    async fn create_note_simple(
        &self,
        title: Option<&str>,
        content: &str,
        created_by: Option<&str>,
    ) -> Result<String> {
        self.insert_note(
            title.unwrap_or(""),
            content,
            "agent",
            created_by.unwrap_or(""),
        ).await
    }

    async fn create_event(&self, e: &NewEvent) -> Result<String> {
        self.create_event(e).await
    }

    async fn list_events_in_range(&self, from: &str, to: &str) -> Result<Vec<Event>> {
        self.list_events_in_range(from, to).await
    }

    async fn insert_reminder(
        &self,
        title: &str,
        remind_at: &str,
        recurrence: Option<&str>,
        target_member: &str,
        created_by: &str,
    ) -> Result<String> {
        self.insert_reminder(title, remind_at, recurrence, target_member, created_by).await
    }

    async fn list_reminders_in_range(&self, from: &str, to: &str) -> Result<Vec<serde_json::Value>> {
        self.list_reminders_active_in_range(from, to).await
    }

    async fn create_scheduled_action(&self, a: &NewScheduledAction) -> Result<String> {
        self.create_scheduled_action(a).await
    }

    async fn list_scheduled_in_range(&self, from: &str, to: &str) -> Result<Vec<ScheduledAction>> {
        // Use SQL-filtered query; map JSON rows back to ScheduledAction via get_scheduled_action
        let rows = self.list_scheduled_active_in_range(from, to).await?;
        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            if let Some(id) = row.get("id").and_then(|v| v.as_str()) {
                if let Ok(Some(action)) = self.get_scheduled_action(id).await {
                    results.push(action);
                }
            }
        }
        Ok(results)
    }

    async fn list_todos_with_deadline(&self, from: &str, to: &str) -> Result<Vec<serde_json::Value>> {
        let rows = self.list_todos_with_deadline_in_range(from, to).await?;
        Ok(rows.into_iter().map(|t| serde_json::to_value(&t).unwrap_or(serde_json::Value::Null)).collect())
    }

    async fn upsert_agent_session(
        &self,
        member_id: &str,
        messages: &[SessionMessage],
        pending: Option<&serde_json::Value>,
    ) -> Result<String> {
        self.upsert_agent_session(member_id, messages, pending).await
    }

    async fn get_active_agent_session(&self, member_id: &str) -> Result<Option<AgentSession>> {
        self.get_active_agent_session(member_id).await
    }

    async fn get_setting(&self, key: &str) -> Result<String> {
        Ok(self.get_setting(key).await?.unwrap_or_else(|| "free".into()))
    }

    async fn get_int_setting(&self, key: &str) -> Result<i64> {
        let val = self.get_setting(key).await?.unwrap_or_default();
        Ok(val.parse().unwrap_or(0))
    }

    async fn increment_int_setting(&self, key: &str, delta: i64) -> Result<()> {
        self.increment_setting_atomic(key, delta).await.map(|_| ())
    }
}
