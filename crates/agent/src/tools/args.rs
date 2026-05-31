//! Typed argument structs for the agent tools.
//!
//! Each tool's arguments are described once, here, by a struct that derives both
//! [`serde::Deserialize`] (the tool's `call()` parses its arguments into it) and
//! [`schemars::JsonSchema`] (the registry derives the tool's `input_schema` from
//! it via [`mcp_types::schema_for_type`]). This is the single source of truth for
//! a tool's argument shape — there is no hand-written JSON schema to drift from.
//!
//! Field docs become JSON-Schema `description`s. `Option<T>` fields are optional;
//! bare `T` fields are `required`. `#[schemars(extend(...))]` adds the schema
//! keywords the model benefits from (`format: date-time`, numeric bounds,
//! defaults), reproducing the previous hand-written schemas.

use schemars::JsonSchema;
use serde::Deserialize;

// ── shared argument enums ────────────────────────────────────────────────────
// Mirror the domain enums with identical serde representations so the derived
// schema advertises the same string values; converted to the domain type in the
// tool body. Kept local to avoid leaking a `schemars` dependency into the
// `memory`/`scheduler` crates.

/// A memory kind. Mirrors `grumps_memory::MemoryKind`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKindArg { Fact, Person, Decision, Preference, Place, Other }

impl From<MemoryKindArg> for grumps_memory::MemoryKind {
    fn from(k: MemoryKindArg) -> Self {
        match k {
            MemoryKindArg::Fact => Self::Fact,
            MemoryKindArg::Person => Self::Person,
            MemoryKindArg::Decision => Self::Decision,
            MemoryKindArg::Preference => Self::Preference,
            MemoryKindArg::Place => Self::Place,
            MemoryKindArg::Other => Self::Other,
        }
    }
}

/// A scheduled-action type. Mirrors `grumps_scheduler::ActionType`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActionTypeArg { Reminder, FollowUp, Recap, AgentTask, EventNotify }

impl From<ActionTypeArg> for grumps_scheduler::ActionType {
    fn from(a: ActionTypeArg) -> Self {
        match a {
            ActionTypeArg::Reminder => Self::Reminder,
            ActionTypeArg::FollowUp => Self::FollowUp,
            ActionTypeArg::Recap => Self::Recap,
            ActionTypeArg::AgentTask => Self::AgentTask,
            ActionTypeArg::EventNotify => Self::EventNotify,
        }
    }
}

/// Web-search recency window: past day/week/month/year/all.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub enum Freshness {
    #[serde(rename = "pd")] Pd,
    #[serde(rename = "pw")] Pw,
    #[serde(rename = "pm")] Pm,
    #[serde(rename = "py")] Py,
    #[serde(rename = "all")] All,
}

impl Freshness {
    pub fn as_str(&self) -> &'static str {
        match self {
            Freshness::Pd => "pd",
            Freshness::Pw => "pw",
            Freshness::Pm => "pm",
            Freshness::Py => "py",
            Freshness::All => "all",
        }
    }
}

/// Calendar item kind to include in a `list_calendar` query.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum CalendarType { Todo, Event, Reminder, Scheduled }

// ── per-tool argument structs ────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueryMemoryArgs {
    /// Free-text search query
    pub query: String,
    /// Optional filter by memory kind
    pub kind: Option<MemoryKindArg>,
    #[schemars(extend("default" = 10))]
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueryChatHistoryArgs {
    pub query: String,
    /// Optional ISO 8601 lower bound
    #[schemars(extend("format" = "date-time"))]
    pub from: Option<String>,
    /// Optional ISO 8601 upper bound
    #[schemars(extend("format" = "date-time"))]
    pub to: Option<String>,
    #[schemars(extend("default" = 5))]
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SaveMemoryArgs {
    /// Short slug, e.g. 'wifi-bureau'
    pub key: Option<String>,
    /// The actual content
    pub value: String,
    pub kind: MemoryKindArg,
    /// member_id if about a person
    pub related_member: Option<String>,
    /// Optional TTL, e.g. for vacation status
    #[schemars(extend("format" = "date-time"))]
    pub expires_at: Option<String>,
    #[schemars(extend("default" = false))]
    pub pinned: Option<bool>,
    /// Optional free-form tags
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateTodoArgs {
    pub title: String,
    /// member_id or display_name
    pub assignee: Option<String>,
    #[schemars(extend("format" = "date-time"))]
    pub deadline: Option<String>,
    /// 1=high, 2=normal, 3=low
    #[schemars(extend("minimum" = 1, "maximum" = 3))]
    pub priority: Option<i64>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompleteTodoArgs {
    /// Natural-language description of the todo to complete
    pub query: Option<String>,
    /// The todo's number (#N)
    pub seq_num: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReopenTodoArgs {
    /// Natural-language description of the completed todo to reopen
    pub query: Option<String>,
    /// The todo's number (#N)
    pub seq_num: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateNoteArgs {
    pub title: Option<String>,
    /// Markdown body
    pub content: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateEventArgs {
    pub title: String,
    /// Local wall-clock time in the group's timezone (see CURRENT DATETIME in
    /// your context), ISO-8601 with NO timezone suffix, e.g. 2026-05-31T20:00:00.
    /// Do not convert to UTC.
    #[schemars(extend("format" = "date-time"))]
    pub starts_at: String,
    /// Optional end time, same local-wall-clock format as starts_at.
    #[schemars(extend("format" = "date-time"))]
    pub ends_at: Option<String>,
    #[schemars(extend("default" = false))]
    pub all_day: Option<bool>,
    pub location: Option<String>,
    /// Free-form description (markdown)
    pub description: Option<String>,
    /// Display color hint
    pub color: Option<String>,
    /// member_ids
    pub attendees: Option<Vec<String>>,
    /// RRULE format, e.g. 'FREQ=WEEKLY;BYDAY=MO'
    pub recurrence: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateReminderArgs {
    /// The reminder message text
    pub text: String,
    /// Local wall-clock time in the group's timezone (see CURRENT DATETIME in
    /// your context), ISO-8601 with NO timezone suffix, e.g. 2026-05-31T20:00:00.
    /// Do not convert to UTC.
    #[schemars(extend("format" = "date-time"))]
    pub trigger_at: String,
    /// Optional RRULE
    pub recurrence: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ScheduleActionArgs {
    pub action_type: ActionTypeArg,
    /// Human summary
    pub title: String,
    /// Local wall-clock time in the group's timezone (see CURRENT DATETIME in
    /// your context), ISO-8601 with NO timezone suffix, e.g. 2026-05-31T20:00:00.
    /// Do not convert to UTC.
    #[schemars(extend("format" = "date-time"))]
    pub trigger_at: String,
    pub recurrence: Option<String>,
    /// Action payload (instruction string for agent_task, etc.)
    pub payload: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListCalendarArgs {
    #[schemars(extend("format" = "date-time"))]
    pub from: String,
    #[schemars(extend("format" = "date-time"))]
    pub to: String,
    pub types: Option<Vec<CalendarType>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WebSearchArgs {
    pub query: String,
    #[schemars(extend("default" = 5, "maximum" = 10))]
    pub count: Option<u64>,
    /// past day/week/month/year/all
    pub freshness: Option<Freshness>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SendMessageArgs {
    pub text: String,
}
