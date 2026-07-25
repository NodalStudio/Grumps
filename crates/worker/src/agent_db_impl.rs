//! impl AgentDb for WorkspaceDb.
//! Bridges the agent's database trait to worker's existing WorkspaceDb methods.

use grumps_agent::db::{
    AgentDb, ChatMessage, LlmCostByModel, LlmErrorEntry, LlmInvocationCount, LlmLatencyByModel,
    MemberShort, NextOccurrenceBrief, NoteBrief, QualitySignalCount, TodoBrief,
};
use grumps_calendar::{Event, NewEvent};
use grumps_memory::{MemoryEntry, NewMemoryEntry};
use grumps_scheduler::{AgentSession, NewScheduledAction, ScheduledAction, SessionMessage};
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
        Ok(rows
            .into_iter()
            .map(|r| MemberShort {
                id: r.id,
                display_name: r.display_name,
                role: r.role,
            })
            .collect())
    }

    async fn get_messages_around(
        &self,
        anchor_id: &str,
        before: i64,
        after: i64,
    ) -> Result<Vec<ChatMessage>> {
        // Disambiguate the inherent WorkspaceDb method from this trait method
        // (same name) to avoid resolving to the trait method recursively.
        let rows = WorkspaceDb::get_messages_around(self, anchor_id, before, after).await?;
        Ok(rows
            .into_iter()
            .map(|r| ChatMessage {
                id: r.id,
                sender_name: r.sender_name,
                text: r.text,
                created_at: r.created_at,
            })
            .collect())
    }

    async fn create_todo_simple(
        &self,
        title: &str,
        assignee_name: Option<&str>,
        deadline: Option<&str>,
        priority: i32,
        tags: Vec<String>,
        created_by: Option<&str>,
    ) -> Result<(String, i64)> {
        let tags_json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".into());
        self.insert_todo(
            title,
            priority,
            &tags_json,
            "", // assigned_to (member id — we only have name here, leave blank)
            assignee_name.unwrap_or(""),
            created_by.unwrap_or(""),
            "agent",
            "",
            deadline, // already a civil "YYYY-MM-DD" (normalized in the tool layer)
        )
        .await
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
        )
        .await
    }

    async fn list_todos(&self, status: &str) -> Result<Vec<serde_json::Value>> {
        self.list_todos_for_agent(status).await
    }

    async fn list_notes(&self, limit: i64) -> Result<Vec<serde_json::Value>> {
        self.list_notes_for_agent(limit).await
    }

    async fn get_todo_by_seq(&self, seq_num: i64) -> Result<Option<TodoBrief>> {
        // Disambiguate from the trait method of the same name.
        let row = WorkspaceDb::get_todo_by_seq(self, seq_num).await?;
        Ok(row.map(|r| TodoBrief {
            id: r.id,
            seq_num: r.seq_num,
            title: r.title,
            status: r.status,
            recurrence: r.recurrence,
        }))
    }

    async fn complete_todo(&self, todo_id: &str, completed_by: &str) -> Result<()> {
        WorkspaceDb::complete_todo(self, todo_id, completed_by).await
    }

    async fn delete_todo(&self, todo_id: &str) -> Result<()> {
        WorkspaceDb::delete_todo(self, todo_id).await
    }

    async fn update_todo(
        &self,
        todo_id: &str,
        title: Option<&str>,
        status: Option<&str>,
        priority: Option<i32>,
        assigned_to: Option<&str>,
        assigned_name: Option<&str>,
    ) -> Result<()> {
        WorkspaceDb::update_todo(
            self,
            todo_id,
            title,
            status,
            priority,
            assigned_to,
            assigned_name,
        )
        .await
    }

    async fn set_todo_deadline(&self, todo_id: &str, deadline: &str) -> Result<()> {
        self.set_todo_deadline(todo_id, deadline).await
    }

    async fn add_todo_tag(&self, todo_id: &str, tag: &str) -> Result<()> {
        self.add_todo_tag(todo_id, tag).await
    }

    async fn create_next_recurrence(
        &self,
        todo: &TodoBrief,
        recurrence: &str,
        tz: chrono_tz::Tz,
    ) -> Result<NextOccurrenceBrief> {
        // WorkspaceDb::create_next_recurrence only reads `todo.id` (it
        // re-fetches title/priority/tags/assignee itself) — a minimal
        // TodoRow built from the brief is enough.
        let row = crate::db::TodoRow {
            id: todo.id.clone(),
            seq_num: todo.seq_num,
            title: todo.title.clone(),
            status: todo.status.clone(),
            recurrence: todo.recurrence.clone(),
        };
        let next = self.create_next_recurrence(&row, recurrence, tz).await?;
        Ok(NextOccurrenceBrief {
            id: next.id,
            seq_num: next.seq_num,
            title: next.title,
            priority: next.priority,
            tags: next.tags,
            assigned_name: next.assigned_name,
            deadline: next.deadline,
        })
    }

    async fn get_note_by_id(&self, note_id: &str) -> Result<Option<NoteBrief>> {
        let row = WorkspaceDb::get_note_by_id(self, note_id).await?;
        Ok(row.map(|r| NoteBrief {
            id: r.id,
            title: r.title,
            content: r.content,
        }))
    }

    async fn get_note_by_title(&self, title: &str) -> Result<Option<NoteBrief>> {
        let row = self.get_note_by_title(title).await?;
        Ok(row.map(|r| NoteBrief {
            id: r.id,
            title: r.title,
            content: r.content,
        }))
    }

    async fn update_note(
        &self,
        note_id: &str,
        title: &str,
        content: &str,
        editor_id: &str,
    ) -> Result<()> {
        WorkspaceDb::update_note(self, note_id, title, content, editor_id).await
    }

    async fn delete_note(&self, note_id: &str) -> Result<()> {
        WorkspaceDb::delete_note(self, note_id).await
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
        self.insert_reminder(title, remind_at, recurrence, target_member, created_by)
            .await
    }

    async fn list_reminders_in_range(
        &self,
        from: &str,
        to: &str,
    ) -> Result<Vec<serde_json::Value>> {
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

    async fn get_scheduled_action(&self, id: &str) -> Result<Option<ScheduledAction>> {
        WorkspaceDb::get_scheduled_action(self, id).await
    }

    async fn update_scheduled_action(
        &self,
        id: &str,
        title: &str,
        trigger_at_iso: &str,
        recurrence: Option<&str>,
        payload: &serde_json::Value,
    ) -> Result<bool> {
        WorkspaceDb::update_scheduled_action(self, id, title, trigger_at_iso, recurrence, payload)
            .await
    }

    async fn delete_scheduled_action(&self, id: &str) -> Result<bool> {
        WorkspaceDb::delete_scheduled_action(self, id).await
    }

    async fn list_todos_with_deadline(
        &self,
        from: &str,
        to: &str,
    ) -> Result<Vec<serde_json::Value>> {
        let rows = self.list_todos_with_deadline_in_range(from, to).await?;
        Ok(rows
            .into_iter()
            .map(|t| serde_json::to_value(&t).unwrap_or(serde_json::Value::Null))
            .collect())
    }

    async fn upsert_agent_session(
        &self,
        member_id: &str,
        messages: &[SessionMessage],
        pending: Option<&serde_json::Value>,
    ) -> Result<String> {
        self.upsert_agent_session(member_id, messages, pending)
            .await
    }

    async fn get_active_agent_session(&self, member_id: &str) -> Result<Option<AgentSession>> {
        self.get_active_agent_session(member_id).await
    }

    async fn log_bot_action(
        &self,
        kind: &str,
        summary: &str,
        target_id: Option<&str>,
    ) -> Result<Option<String>> {
        self.log_bot_action(kind, summary, target_id).await
    }

    async fn list_recent_bot_actions(
        &self,
        max_age_seconds: i64,
        limit: i64,
    ) -> Result<Vec<grumps_agent::ambient::RecentBotAction>> {
        self.list_recent_bot_actions(max_age_seconds, limit).await
    }

    async fn log_quality_signal(
        &self,
        member_id: &str,
        signal_type: &str,
        target_activity_id: Option<&str>,
        target_activity_type: Option<&str>,
        raw_text: &str,
        confidence: f64,
        reason: &str,
    ) -> Result<()> {
        self.log_quality_signal(
            member_id,
            signal_type,
            target_activity_id,
            target_activity_type,
            raw_text,
            confidence,
            reason,
        )
        .await
    }

    async fn get_setting(&self, key: &str) -> Result<String> {
        // Missing → empty string. Callers apply their own per-key default
        // (e.g. Plan::from_str("") == Free). A hardcoded "free" here leaked the
        // plan default into unrelated keys (e.g. timezone, persona).
        Ok(self.get_setting(key).await?.unwrap_or_default())
    }

    async fn get_all_settings(&self) -> Result<std::collections::HashMap<String, String>> {
        self.get_all_settings().await
    }

    async fn get_int_setting(&self, key: &str) -> Result<i64> {
        let val = self.get_setting(key).await?.unwrap_or_default();
        Ok(val.parse().unwrap_or(0))
    }

    async fn increment_int_setting(&self, key: &str, delta: i64) -> Result<()> {
        self.increment_setting_atomic(key, delta).await.map(|_| ())
    }

    async fn log_llm_call(&self, record: &grumps_agent::telemetry::LlmCallRecord) -> Result<()> {
        self.log_llm_call(record).await
    }

    async fn aggregate_llm_costs_30d(&self) -> Result<Vec<LlmCostByModel>> {
        self.aggregate_llm_costs_30d().await
    }

    async fn aggregate_llm_latency_by_model(&self) -> Result<Vec<LlmLatencyByModel>> {
        self.aggregate_llm_latency_by_model().await
    }

    async fn aggregate_llm_invocation_types(&self) -> Result<Vec<LlmInvocationCount>> {
        self.aggregate_llm_invocation_types().await
    }

    async fn list_recent_llm_errors(&self, limit: i64) -> Result<Vec<LlmErrorEntry>> {
        self.list_recent_llm_errors(limit).await
    }

    async fn aggregate_quality_signals_30d(&self) -> Result<Vec<QualitySignalCount>> {
        self.aggregate_quality_signals_30d().await
    }
}
