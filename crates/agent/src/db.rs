//! AgentDb trait — the minimal DB surface the agent tool layer needs.
//! Worker implements this on WorkspaceDb via `crates/worker/src/agent_db_impl.rs`.

use grumps_calendar::{Event, NewEvent};
use grumps_memory::{MemoryEntry, NewMemoryEntry};
use grumps_scheduler::{AgentSession, NewScheduledAction, ScheduledAction, SessionMessage};
use serde::{Deserialize, Serialize};

// ── Observability response types ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCostByModel {
    pub provider: String,
    pub model: String,
    pub cost_usd: f64,
    pub call_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmLatencyByModel {
    pub provider: String,
    pub model: String,
    pub p50_ms: i64,
    pub p95_ms: i64,
    pub p99_ms: i64,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmInvocationCount {
    pub invocation_type: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmErrorEntry {
    pub created_at: String,
    pub provider: String,
    pub model: String,
    pub error: String,
    pub invocation_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualitySignalCount {
    pub signal_type: String,
    pub count: i64,
}

/// A short member record for prompt context.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemberShort {
    pub id: String,
    pub display_name: Option<String>,
    pub role: String,
}

/// A chat-history message, used to build conversational context windows around
/// a RAG match. `id` is the message's UUIDv7 (time-ordered) anchor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub sender_name: Option<String>,
    pub text: String,
    pub created_at: String,
}

/// Minimal todo row for seq→id resolution and mutation tools
/// (`complete_todo`/`update_todo`/`delete_todo`). `status` lets a tool refuse
/// a no-op (e.g. completing an already-done todo); `recurrence` drives the
/// same next-occurrence spawn the command path's Done arm performs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoBrief {
    pub id: String,
    pub seq_num: i64,
    pub title: String,
    pub status: String,
    pub recurrence: Option<String>,
}

/// The newly-spawned occurrence of a recurring todo — mirrors the worker's
/// `db::NextOccurrence`, shaped so the tool layer can push it onto
/// `ToolContext::created_todos` and get the exact same task-card rendering
/// `create_todo` gets (see `tools::crud::complete_todo`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextOccurrenceBrief {
    pub id: String,
    pub seq_num: i64,
    pub title: String,
    pub priority: i32,
    pub tags: Vec<String>,
    pub assigned_name: Option<String>,
    pub deadline: Option<String>,
}

/// Minimal note row for `update_note`/`delete_note`'s id-or-title resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteBrief {
    pub id: String,
    pub title: Option<String>,
    pub content: String,
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
    /// Returns `(todo_id, seq_num)` — the seq is needed to render the same
    /// task card the deterministic parser path produces.
    async fn create_todo_simple(
        &self,
        title: &str,
        assignee_name: Option<&str>,
        deadline: Option<&str>,
        priority: i32,
        tags: Vec<String>,
        created_by: Option<&str>,
    ) -> worker::Result<(String, i64)>;

    async fn create_note_simple(
        &self,
        title: Option<&str>,
        content: &str,
        created_by: Option<&str>,
    ) -> worker::Result<String>;

    /// List todos for the `list_todos` tool. `status` is "open"/"done"/"all".
    async fn list_todos(&self, status: &str) -> worker::Result<Vec<serde_json::Value>>;

    /// List notes for the `list_notes` tool, most recent first, capped at `limit`.
    async fn list_notes(&self, limit: i64) -> worker::Result<Vec<serde_json::Value>>;

    // --- todo mutation (complete_todo/update_todo/delete_todo tools) ---
    /// Resolve a human-facing seq number to the row mutation tools need.
    async fn get_todo_by_seq(&self, seq_num: i64) -> worker::Result<Option<TodoBrief>>;
    /// Mark done — same call the deterministic card-reply Done arm uses.
    async fn complete_todo(&self, todo_id: &str, completed_by: &str) -> worker::Result<()>;
    async fn delete_todo(&self, todo_id: &str) -> worker::Result<()>;
    /// Partial update — `None` leaves a field unchanged (same CASE-WHEN
    /// semantics as the card-reply Edit/Reassign/ChangePriority/ChangeStatus
    /// arms, which all share this one call).
    #[allow(clippy::too_many_arguments)]
    async fn update_todo(
        &self,
        todo_id: &str,
        title: Option<&str>,
        status: Option<&str>,
        priority: Option<i32>,
        assigned_to: Option<&str>,
        assigned_name: Option<&str>,
    ) -> worker::Result<()>;
    /// Set (non-empty) or clear (empty string) a todo's deadline.
    async fn set_todo_deadline(&self, todo_id: &str, deadline: &str) -> worker::Result<()>;
    /// Append one normalized tag (dedup, trim+lowercase) — same call the
    /// card-reply AddTag arm uses.
    async fn add_todo_tag(&self, todo_id: &str, tag: &str) -> worker::Result<()>;
    /// Spawn the next occurrence of a recurring todo — same call the
    /// card-reply Done arm uses when the completed todo has a recurrence.
    async fn create_next_recurrence(
        &self,
        todo: &TodoBrief,
        recurrence: &str,
        tz: chrono_tz::Tz,
    ) -> worker::Result<NextOccurrenceBrief>;

    // --- note mutation (update_note/delete_note tools) ---
    async fn get_note_by_id(&self, note_id: &str) -> worker::Result<Option<NoteBrief>>;
    /// Case/whitespace-insensitive title lookup (same `title_norm` column the
    /// wikilink resolver uses) — the id-or-title fallback for `update_note`/
    /// `delete_note` when the model only has the note's title.
    async fn get_note_by_title(&self, title: &str) -> worker::Result<Option<NoteBrief>>;
    async fn update_note(
        &self,
        note_id: &str,
        title: &str,
        content: &str,
        editor_id: &str,
    ) -> worker::Result<()>;
    async fn delete_note(&self, note_id: &str) -> worker::Result<()>;

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
    async fn list_reminders_in_range(
        &self,
        from: &str,
        to: &str,
    ) -> worker::Result<Vec<serde_json::Value>>;

    // --- scheduled actions ---
    async fn create_scheduled_action(&self, a: &NewScheduledAction) -> worker::Result<String>;
    async fn list_scheduled_in_range(
        &self,
        from: &str,
        to: &str,
    ) -> worker::Result<Vec<ScheduledAction>>;
    /// Fetch one scheduled action by id (`cancel_scheduled`/`update_scheduled`
    /// read the existing row first so a partial update can fall back to the
    /// current title/trigger_at/recurrence for omitted fields).
    async fn get_scheduled_action(&self, id: &str) -> worker::Result<Option<ScheduledAction>>;
    /// Update title/trigger_at/recurrence/payload of an existing action.
    /// Returns `false` if no row matched `id`. The DO alarm is re-armed by
    /// the caller after the agent run returns (see `route_via_agent` /
    /// `scheduler_executor::execute_action`), not here.
    async fn update_scheduled_action(
        &self,
        id: &str,
        title: &str,
        trigger_at_iso: &str,
        recurrence: Option<&str>,
        payload: &serde_json::Value,
    ) -> worker::Result<bool>;
    /// Returns `false` if no row matched `id`.
    async fn delete_scheduled_action(&self, id: &str) -> worker::Result<bool>;

    // --- todos with deadlines (for calendar aggregation) ---
    async fn list_todos_with_deadline(
        &self,
        from: &str,
        to: &str,
    ) -> worker::Result<Vec<serde_json::Value>>;

    // --- settings ---
    async fn get_setting(&self, key: &str) -> worker::Result<String>;
    async fn get_int_setting(&self, key: &str) -> worker::Result<i64>;
    async fn increment_int_setting(&self, key: &str, delta: i64) -> worker::Result<()>;
    /// Fetch every setting row in one query. Lets the prompt builder assemble
    /// its context with a single round-trip instead of one REST call per key.
    async fn get_all_settings(&self) -> worker::Result<std::collections::HashMap<String, String>>;

    // --- bot activity + quality signals ---
    async fn log_bot_action(
        &self,
        kind: &str,
        summary: &str,
        target_id: Option<&str>,
    ) -> worker::Result<Option<String>>;
    async fn list_recent_bot_actions(
        &self,
        max_age_seconds: i64,
        limit: i64,
    ) -> worker::Result<Vec<crate::ambient::RecentBotAction>>;
    async fn log_quality_signal(
        &self,
        member_id: &str,
        signal_type: &str,
        target_activity_id: Option<&str>,
        target_activity_type: Option<&str>,
        raw_text: &str,
        confidence: f64,
        reason: &str,
    ) -> worker::Result<()>;

    // --- chat history (RAG context windows) ---
    /// Fetch the messages around an anchor message id (chronological order,
    /// anchor included): `before` strictly-earlier messages and `after`
    /// messages from the anchor onward.
    async fn get_messages_around(
        &self,
        anchor_id: &str,
        before: i64,
        after: i64,
    ) -> worker::Result<Vec<ChatMessage>>;

    // --- agent sessions ---
    async fn upsert_agent_session(
        &self,
        member_id: &str,
        messages: &[SessionMessage],
        pending: Option<&serde_json::Value>,
    ) -> worker::Result<String>;
    async fn get_active_agent_session(
        &self,
        member_id: &str,
    ) -> worker::Result<Option<AgentSession>>;

    // --- LLM observability ---
    async fn log_llm_call(&self, record: &crate::telemetry::LlmCallRecord) -> worker::Result<()>;
    async fn aggregate_llm_costs_30d(&self) -> worker::Result<Vec<LlmCostByModel>>;
    async fn aggregate_llm_latency_by_model(&self) -> worker::Result<Vec<LlmLatencyByModel>>;
    async fn aggregate_llm_invocation_types(&self) -> worker::Result<Vec<LlmInvocationCount>>;
    async fn list_recent_llm_errors(&self, limit: i64) -> worker::Result<Vec<LlmErrorEntry>>;
    async fn aggregate_quality_signals_30d(&self) -> worker::Result<Vec<QualitySignalCount>>;
}
