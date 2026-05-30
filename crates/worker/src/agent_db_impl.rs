//! impl AgentDb for WorkspaceDb.
//! Bridges the agent's database trait to worker's existing WorkspaceDb methods.

use grumps_agent::db::{AgentDb, MemberShort, LlmCostByModel, LlmLatencyByModel, LlmInvocationCount, LlmErrorEntry, QualitySignalCount};
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
        let (id, _seq) = self.insert_todo(
            title,
            priority,
            &tags_json,
            "", // assigned_to (member id — we only have name here, leave blank)
            assignee_name.unwrap_or(""),
            created_by.unwrap_or(""),
            "agent",
            "",
            deadline, // already a civil "YYYY-MM-DD" (normalized in the tool layer)
        ).await?;

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

    async fn list_open_todos(&self) -> Result<Vec<(String, String, i64)>> {
        self.list_open_todos_brief().await
    }

    async fn list_done_todos(&self) -> Result<Vec<(String, String, i64)>> {
        self.list_done_todos_brief().await
    }

    async fn complete_todo(&self, todo_id: &str, completed_by: &str) -> Result<()> {
        self.complete_todo(todo_id, completed_by).await
    }

    async fn reopen_todo(&self, todo_id: &str) -> Result<()> {
        self.reopen_todo(todo_id).await
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

    async fn log_bot_action(&self, kind: &str, summary: &str, target_id: Option<&str>) -> Result<Option<String>> {
        self.log_bot_action(kind, summary, target_id).await
    }

    async fn list_recent_bot_actions(&self, max_age_seconds: i64, limit: i64) -> Result<Vec<grumps_agent::ambient::RecentBotAction>> {
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
        self.log_quality_signal(member_id, signal_type, target_activity_id, target_activity_type, raw_text, confidence, reason).await
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
