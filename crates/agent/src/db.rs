//! AgentDb trait — the minimal DB surface the agent tool layer needs.
//! Worker implements this on WorkspaceDb via `crates/worker/src/agent_db_impl.rs`.

use grumps_memory::{MemoryEntry, NewMemoryEntry};
use grumps_calendar::{Event, NewEvent};
use grumps_scheduler::{NewScheduledAction, ScheduledAction, AgentSession, SessionMessage};

/// A short member record for prompt context.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemberShort {
    pub id: String,
    pub display_name: Option<String>,
    pub role: String,
}

#[async_trait::async_trait(?Send)]
pub trait AgentDb {
    // --- memory ---
    async fn list_pinned_memory(&self) -> worker::Result<Vec<MemoryEntry>>;
    async fn search_memory_fts(&self, query: &str, limit: i64) -> worker::Result<Vec<MemoryEntry>>;
    async fn create_memory(&self, entry: &NewMemoryEntry) -> worker::Result<String>;

    // --- members ---
    async fn list_active_members(&self) -> worker::Result<Vec<MemberShort>>;

    // --- todos / notes ---
    async fn create_todo_simple(
        &self,
        title: &str,
        assignee_name: Option<&str>,
        deadline: Option<&str>,
        priority: i32,
        tags: Vec<String>,
        created_by: Option<&str>,
    ) -> worker::Result<String>;

    async fn create_note_simple(
        &self,
        title: Option<&str>,
        content: &str,
        created_by: Option<&str>,
    ) -> worker::Result<String>;

    // --- events ---
    async fn create_event(&self, e: &NewEvent) -> worker::Result<String>;
    async fn list_events_in_range(&self, from: &str, to: &str) -> worker::Result<Vec<Event>>;

    // --- reminders ---
    async fn insert_reminder(
        &self,
        title: &str,
        remind_at: &str,
        recurrence: Option<&str>,
        target_member: &str,
        created_by: &str,
    ) -> worker::Result<String>;

    /// List active reminders in a datetime range (remind_at between from and to).
    async fn list_reminders_in_range(&self, from: &str, to: &str) -> worker::Result<Vec<serde_json::Value>>;

    // --- scheduled actions ---
    async fn create_scheduled_action(&self, a: &NewScheduledAction) -> worker::Result<String>;
    async fn list_scheduled_in_range(&self, from: &str, to: &str) -> worker::Result<Vec<ScheduledAction>>;

    // --- todos with deadlines (for calendar aggregation) ---
    async fn list_todos_with_deadline(&self, from: &str, to: &str) -> worker::Result<Vec<serde_json::Value>>;

    // --- settings ---
    async fn get_setting(&self, key: &str) -> worker::Result<String>;
    async fn get_int_setting(&self, key: &str) -> worker::Result<i64>;
    async fn increment_int_setting(&self, key: &str, delta: i64) -> worker::Result<()>;

    // --- agent sessions ---
    async fn upsert_agent_session(
        &self,
        member_id: &str,
        messages: &[SessionMessage],
        pending: Option<&serde_json::Value>,
    ) -> worker::Result<String>;
    async fn get_active_agent_session(&self, member_id: &str) -> worker::Result<Option<AgentSession>>;
}
