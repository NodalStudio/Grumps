//! Tool registry: the single source of truth for which tools exist, their MCP
//! descriptors (incl. behavioural annotations), and their dispatch.
//!
//! Each tool is a [`ToolHandler`]. A handler exposes an `mcp_types::Tool`
//! descriptor — name, description, JSON-Schema of its arguments, and
//! `ToolAnnotations` (read-only / destructive hints). The annotations are what
//! the proactive path uses to decide which tools may run autonomously vs. which
//! must be proposed-then-confirmed.
//!
//! Two projections are derived from the registry:
//! - [`anthropic_tools`] — the tool list sent to the Anthropic API. We project
//!   `mcp_types::Tool` down to `{ name, description, input_schema }` (snake_case,
//!   no annotations) because that is the shape the API expects.
//! - [`dispatch`] — route a tool call by name to its handler.
//!
//! The input schemas currently come from `schemas.rs` (hand-written JSON). Tools
//! are migrated to `#[derive(JsonSchema)]` argument structs incrementally; until
//! then a handler's `descriptor()` lifts the existing schema into an
//! `mcp_types::Tool`.

use mcp_types::{Tool, ToolAnnotations};
use serde_json::Value;

use super::{args, ToolContext};
use super::{calendar, chat, crud, members, memory, rag, scheduler, todos, web};

/// An executable tool: its MCP descriptor plus its handler.
#[async_trait::async_trait(?Send)]
pub trait ToolHandler {
    /// The tool's invariant name (matches `descriptor().name`).
    fn name(&self) -> &'static str;
    /// The behavioural annotations (read-only / destructive hints). Exposed
    /// directly so the proactive gate can read them without building the full
    /// `descriptor()` (which materializes the argument JSON-Schema).
    fn annotations(&self) -> Option<ToolAnnotations>;
    /// The MCP descriptor (name, schema, annotations) for this tool.
    fn descriptor(&self) -> Tool;
    /// Execute the tool. The returned JSON becomes the `tool_result` content.
    async fn call(&self, ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value>;
}

/// Declare a `ToolHandler` unit struct. Its `input_schema` is derived from the
/// typed argument struct `$args` (the single source of truth — see `args.rs`),
/// and `call` delegates to the tool function.
macro_rules! handler {
    ($ty:ident, $name:literal, $desc:literal, $args:ty, $annotations:expr, $call:path) => {
        pub struct $ty;
        #[async_trait::async_trait(?Send)]
        impl ToolHandler for $ty {
            fn name(&self) -> &'static str {
                $name
            }
            fn annotations(&self) -> Option<ToolAnnotations> {
                Some($annotations)
            }
            fn descriptor(&self) -> Tool {
                Tool::new($name, $desc, mcp_types::schema_for_type::<$args>())
                    .with_annotations($annotations)
            }
            async fn call(&self, ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
                $call(ctx, args).await
            }
        }
    };
}

// Read-only tools — safe to run autonomously in proactive mode.
handler!(QueryMemory, "query_memory",
    "Search the workspace's structured memory (facts, people, decisions, preferences). Use when the user asks about something the group knows or to recall who/what/when.",
    args::QueryMemoryArgs, ToolAnnotations::read_only(), memory::query_memory);
handler!(QueryChatHistory, "query_chat_history",
    "Semantic search over past chat messages in this group. Use when the user asks 'what did we say about...', 'when did X happen', etc.",
    args::QueryChatHistoryArgs, ToolAnnotations::read_only(), rag::query_chat_history);
handler!(
    ListCalendar,
    "list_calendar",
    "Read upcoming todos, reminders, events for a date range.",
    args::ListCalendarArgs,
    ToolAnnotations::read_only(),
    calendar::list_calendar
);
handler!(GetTodoStatus, "get_todo_status",
    "Check the current status (open/done) of a specific todo. Use this to judge a conditional instruction like 'remind them only if the trash todo isn't done yet' before acting. Provide its seq_num (#N) OR a natural-language `query`.",
    args::GetTodoStatusArgs, ToolAnnotations::read_only(), todos::get_todo_status);
handler!(GetMemberActivity, "get_member_activity",
    "Look up when a group member was last active. Use this to judge a conditional instruction like 'ping Alice only if she's been silent since yesterday' before acting. Provide the member's name.",
    args::MemberActivityArgs, ToolAnnotations::read_only(), members::get_member_activity);
handler!(WebSearch, "web_search",
    "Search the web. Use for current information not in workspace memory (restaurants, addresses, news, prices, hours).",
    args::WebSearchArgs,
    ToolAnnotations { read_only_hint: Some(true), open_world_hint: Some(true), ..ToolAnnotations::default() },
    web::web_search);

// Communication — not a data mutation; it is the agent's voice and is how a
// proactive proposal is delivered, so it is allowed autonomously.
handler!(SendMessage, "send_message",
    "Send a formatted message to the group chat. Most responses are returned implicitly via your final text — use this only to send extra messages mid-flow.",
    args::SendMessageArgs, ToolAnnotations::read_only(), chat::send_message);

// State-mutating tools — additive, non-destructive. In proactive mode these are
// proposed-then-confirmed rather than executed outright.
const MUTATING: fn() -> ToolAnnotations = || ToolAnnotations {
    read_only_hint: Some(false),
    destructive_hint: Some(false),
    ..ToolAnnotations::default()
};
handler!(SaveMemory, "save_memory",
    "Persist a fact, decision, person info, or preference for this workspace. Use sparingly — only for things that will matter later.",
    args::SaveMemoryArgs, MUTATING(), memory::save_memory);
handler!(
    CreateTodo,
    "create_todo",
    "Create a new todo item in the group's list.",
    args::CreateTodoArgs,
    MUTATING(),
    crud::create_todo
);
// Reversible mutations: not destructive (each has an inverse — complete↔reopen),
// so they may be proposed-then-confirmed in proactive mode and undone after.
handler!(CompleteTodo, "complete_todo",
    "Mark an existing todo as done. Provide its seq_num (the #N number, preferred when known) OR a natural-language `query` describing it — fuzzy matching resolves the todo. If the match is ambiguous the result lists candidates to choose from.",
    args::CompleteTodoArgs, MUTATING(), todos::complete_todo);
handler!(ReopenTodo, "reopen_todo",
    "Reopen a previously completed todo (undo a completion). Provide its seq_num OR a natural-language `query` describing it.",
    args::ReopenTodoArgs, MUTATING(), todos::reopen_todo);
handler!(
    CreateNote,
    "create_note",
    "Create a new note (markdown content) in the group's notes.",
    args::CreateNoteArgs,
    MUTATING(),
    crud::create_note
);
handler!(
    CreateEvent,
    "create_event",
    "Create a calendar event (meeting, appointment, birthday, etc).",
    args::CreateEventArgs,
    MUTATING(),
    crud::create_event
);
handler!(
    CreateReminder,
    "create_reminder",
    "Schedule a passive reminder message in the group at a future time.",
    args::CreateReminderArgs,
    MUTATING(),
    crud::create_reminder
);
handler!(ScheduleAction, "schedule_action",
    "Schedule a complex agentic task to run later (e.g. weekly recap, conditional follow-up). The agent will run autonomously at trigger_at.",
    args::ScheduleActionArgs, MUTATING(), scheduler::schedule_action);

/// The ordered tool registry.
pub fn registry() -> Vec<Box<dyn ToolHandler>> {
    vec![
        Box::new(QueryMemory),
        Box::new(QueryChatHistory),
        Box::new(SaveMemory),
        Box::new(CreateTodo),
        Box::new(CompleteTodo),
        Box::new(ReopenTodo),
        Box::new(CreateNote),
        Box::new(CreateEvent),
        Box::new(CreateReminder),
        Box::new(ScheduleAction),
        Box::new(ListCalendar),
        Box::new(GetTodoStatus),
        Box::new(GetMemberActivity),
        Box::new(WebSearch),
        Box::new(SendMessage),
    ]
}

/// The tool list sent to the Anthropic API: `{ name, description, input_schema }`.
/// Annotations stay internal (used for proactive gating, not sent to the model).
///
/// The list is immutable for the lifetime of the isolate, so it is built once
/// and cached — every agent turn would otherwise re-derive 13 descriptors and
/// their JSON-Schemas.
pub fn anthropic_tools() -> Vec<Value> {
    thread_local! {
        static CACHE: std::cell::OnceCell<Vec<Value>> = const { std::cell::OnceCell::new() };
    }
    CACHE.with(|c| {
        c.get_or_init(|| {
            registry()
                .iter()
                .map(|h| {
                    let t = h.descriptor();
                    serde_json::json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.input_schema,
                    })
                })
                .collect()
        })
        .clone()
    })
}

/// Dispatch a tool call to its handler.
pub async fn dispatch(
    ctx: &ToolContext<'_>,
    tool_name: &str,
    args: Value,
) -> worker::Result<Value> {
    for handler in registry() {
        if handler.name() == tool_name {
            return handler.call(ctx, args).await;
        }
    }
    Err(worker::Error::RustError(format!(
        "unknown tool: {tool_name}"
    )))
}

/// The behavioural annotations for a tool, by name (used by the proactive gate).
pub fn annotations(tool_name: &str) -> Option<ToolAnnotations> {
    registry()
        .iter()
        .find(|h| h.name() == tool_name)
        .and_then(|h| h.annotations())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_covers_all_named_tools() {
        let names: Vec<&str> = registry().iter().map(|h| h.name()).collect();
        assert_eq!(names.len(), 15);
        let unique: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(unique.len(), names.len(), "duplicate tool names");
    }

    #[test]
    fn mutating_todo_tools_registered() {
        assert_eq!(
            annotations("complete_todo").unwrap().read_only_hint,
            Some(false)
        );
        assert_eq!(
            annotations("complete_todo").unwrap().destructive_hint,
            Some(false)
        );
        assert_eq!(
            annotations("reopen_todo").unwrap().destructive_hint,
            Some(false)
        );
    }

    #[test]
    fn anthropic_tools_have_snake_case_input_schema() {
        for t in anthropic_tools() {
            assert!(t.get("name").and_then(|v| v.as_str()).is_some());
            assert!(t.get("description").is_some());
            // The Anthropic API expects snake_case `input_schema`, never the
            // MCP camelCase `inputSchema`, and no `annotations`.
            assert!(t.get("input_schema").and_then(|v| v.as_object()).is_some());
            assert!(t.get("inputSchema").is_none());
            assert!(t.get("annotations").is_none());
        }
    }

    #[test]
    fn read_only_tools_flagged_for_autonomy() {
        assert_eq!(
            annotations("query_memory").unwrap().read_only_hint,
            Some(true)
        );
        assert_eq!(
            annotations("list_calendar").unwrap().read_only_hint,
            Some(true)
        );
        assert_eq!(
            annotations("web_search").unwrap().open_world_hint,
            Some(true)
        );
    }

    #[test]
    fn mutating_tools_not_read_only() {
        assert_eq!(
            annotations("create_todo").unwrap().read_only_hint,
            Some(false)
        );
        assert_eq!(
            annotations("create_reminder").unwrap().read_only_hint,
            Some(false)
        );
        assert_eq!(
            annotations("schedule_action").unwrap().read_only_hint,
            Some(false)
        );
    }

    #[test]
    fn unknown_tool_has_no_annotations() {
        assert!(annotations("does_not_exist").is_none());
    }

    // ── derived-schema fidelity ──────────────────────────────────────────────

    /// The `input_schema` object for a tool, by name.
    fn schema_of(name: &str) -> Value {
        let h = registry()
            .into_iter()
            .find(|h| h.name() == name)
            .expect("tool exists");
        Value::Object((*h.descriptor().input_schema).clone())
    }

    fn required(schema: &Value) -> Vec<String> {
        schema
            .get("required")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn every_tool_schema_is_object_with_properties() {
        for h in registry() {
            let s = Value::Object((*h.descriptor().input_schema).clone());
            assert_eq!(
                s.get("type").and_then(|v| v.as_str()),
                Some("object"),
                "{}",
                h.name()
            );
            assert!(
                s.get("properties").and_then(|p| p.as_object()).is_some(),
                "{}",
                h.name()
            );
        }
    }

    #[test]
    fn required_fields_match_argument_structs() {
        // Bare (non-Option) fields are required; Option<T> fields are not.
        assert_eq!(required(&schema_of("create_todo")), vec!["title"]);
        assert!(required(&schema_of("complete_todo")).is_empty()); // both args optional
        let mut sm = required(&schema_of("save_memory"));
        sm.sort();
        assert_eq!(sm, vec!["kind", "value"]);
        let mut sa = required(&schema_of("schedule_action"));
        sa.sort();
        assert_eq!(sa, vec!["action_type", "payload", "title", "trigger_at"]);
        assert_eq!(required(&schema_of("send_message")), vec!["text"]);
    }

    #[test]
    fn enum_values_are_inlined_faithfully() {
        let kind = schema_of("save_memory");
        let kind = kind.get("properties").and_then(|p| p.get("kind")).unwrap();
        let vals: Vec<&str> = kind
            .get("enum")
            .and_then(|e| e.as_array())
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        for k in ["fact", "person", "decision", "preference", "place", "other"] {
            assert!(vals.contains(&k), "kind enum missing {k}: {vals:?}");
        }

        let ws = schema_of("web_search");
        let fr = ws
            .get("properties")
            .and_then(|p| p.get("freshness"))
            .unwrap();
        let vals: Vec<&str> = fr
            .get("enum")
            .and_then(|e| e.as_array())
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        for f in ["pd", "pw", "pm", "py", "all"] {
            assert!(vals.contains(&f), "freshness enum missing {f}: {vals:?}");
        }
    }

    #[test]
    fn datetime_fields_carry_format_and_description() {
        let ev = schema_of("create_event");
        let starts = ev
            .get("properties")
            .and_then(|p| p.get("starts_at"))
            .unwrap();
        assert_eq!(
            starts.get("format").and_then(|v| v.as_str()),
            Some("date-time")
        );
        // Field doc-comment survives as the schema description.
        assert!(
            starts
                .get("description")
                .and_then(|v| v.as_str())
                .map(|d| d.contains("wall-clock"))
                .unwrap_or(false),
            "starts_at desc: {starts:?}"
        );
    }

    #[test]
    fn numeric_bounds_preserved() {
        let ct = schema_of("create_todo");
        let prio = ct
            .get("properties")
            .and_then(|p| p.get("priority"))
            .unwrap();
        assert_eq!(prio.get("minimum").and_then(|v| v.as_i64()), Some(1));
        assert_eq!(prio.get("maximum").and_then(|v| v.as_i64()), Some(3));
    }
}
