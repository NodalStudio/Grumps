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

use std::sync::Arc;

use mcp_types::{Tool, ToolAnnotations};
use serde_json::Value;

use super::{schemas, ToolContext};
use super::{crud, todos, scheduler, calendar, memory, rag, web, chat};

/// An executable tool: its MCP descriptor plus its handler.
#[async_trait::async_trait(?Send)]
pub trait ToolHandler {
    /// The tool's invariant name (matches `descriptor().name`).
    fn name(&self) -> &'static str;
    /// The MCP descriptor (name, schema, annotations) for this tool.
    fn descriptor(&self) -> Tool;
    /// Execute the tool. The returned JSON becomes the `tool_result` content.
    async fn call(&self, ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value>;
}

/// Build an `mcp_types::Tool` descriptor from an existing `schemas.rs` entry
/// (transitional — lifts the hand-written `input_schema` into the MCP model).
fn lift(name: &'static str, raw: Value, annotations: ToolAnnotations) -> Tool {
    let description = raw
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let input_schema = raw
        .get("input_schema")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    Tool::new(name, description, Arc::new(input_schema)).with_annotations(annotations)
}

/// Declare a `ToolHandler` unit struct that lifts its schema from `schemas.rs`
/// and delegates `call` to an existing tool function.
macro_rules! handler {
    ($ty:ident, $name:literal, $schema:path, $annotations:expr, $call:path) => {
        pub struct $ty;
        #[async_trait::async_trait(?Send)]
        impl ToolHandler for $ty {
            fn name(&self) -> &'static str {
                $name
            }
            fn descriptor(&self) -> Tool {
                lift($name, $schema(), $annotations)
            }
            async fn call(&self, ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
                $call(ctx, args).await
            }
        }
    };
}

// Read-only tools — safe to run autonomously in proactive mode.
handler!(QueryMemory, "query_memory", schemas::query_memory, ToolAnnotations::read_only(), memory::query_memory);
handler!(QueryChatHistory, "query_chat_history", schemas::query_chat_history, ToolAnnotations::read_only(), rag::query_chat_history);
handler!(ListCalendar, "list_calendar", schemas::list_calendar, ToolAnnotations::read_only(), calendar::list_calendar);
handler!(WebSearch, "web_search", schemas::web_search,
    ToolAnnotations { read_only_hint: Some(true), open_world_hint: Some(true), ..ToolAnnotations::default() },
    web::web_search);

// Communication — not a data mutation; it is the agent's voice and is how a
// proactive proposal is delivered, so it is allowed autonomously.
handler!(SendMessage, "send_message", schemas::send_message, ToolAnnotations::read_only(), chat::send_message);

// State-mutating tools — additive, non-destructive. In proactive mode these are
// proposed-then-confirmed rather than executed outright.
const MUTATING: fn() -> ToolAnnotations = || ToolAnnotations {
    read_only_hint: Some(false),
    destructive_hint: Some(false),
    ..ToolAnnotations::default()
};
handler!(SaveMemory, "save_memory", schemas::save_memory, MUTATING(), memory::save_memory);
handler!(CreateTodo, "create_todo", schemas::create_todo, MUTATING(), crud::create_todo);
// Reversible mutations: not destructive (each has an inverse — complete↔reopen),
// so they may be proposed-then-confirmed in proactive mode and undone after.
handler!(CompleteTodo, "complete_todo", schemas::complete_todo, MUTATING(), todos::complete_todo);
handler!(ReopenTodo, "reopen_todo", schemas::reopen_todo, MUTATING(), todos::reopen_todo);
handler!(CreateNote, "create_note", schemas::create_note, MUTATING(), crud::create_note);
handler!(CreateEvent, "create_event", schemas::create_event, MUTATING(), crud::create_event);
handler!(CreateReminder, "create_reminder", schemas::create_reminder, MUTATING(), crud::create_reminder);
handler!(ScheduleAction, "schedule_action", schemas::schedule_action, MUTATING(), scheduler::schedule_action);

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
        Box::new(WebSearch),
        Box::new(SendMessage),
    ]
}

/// The tool list sent to the Anthropic API: `{ name, description, input_schema }`.
/// Annotations stay internal (used for proactive gating, not sent to the model).
pub fn anthropic_tools() -> Vec<Value> {
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
    Err(worker::Error::RustError(format!("unknown tool: {tool_name}")))
}

/// The behavioural annotations for a tool, by name (used by the proactive gate).
pub fn annotations(tool_name: &str) -> Option<ToolAnnotations> {
    registry()
        .iter()
        .find(|h| h.name() == tool_name)
        .and_then(|h| h.descriptor().annotations)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_covers_all_named_tools() {
        let names: Vec<&str> = registry().iter().map(|h| h.name()).collect();
        assert_eq!(names.len(), 13);
        let unique: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(unique.len(), names.len(), "duplicate tool names");
    }

    #[test]
    fn mutating_todo_tools_registered() {
        assert_eq!(annotations("complete_todo").unwrap().read_only_hint, Some(false));
        assert_eq!(annotations("complete_todo").unwrap().destructive_hint, Some(false));
        assert_eq!(annotations("reopen_todo").unwrap().destructive_hint, Some(false));
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
        assert_eq!(annotations("query_memory").unwrap().read_only_hint, Some(true));
        assert_eq!(annotations("list_calendar").unwrap().read_only_hint, Some(true));
        assert_eq!(annotations("web_search").unwrap().open_world_hint, Some(true));
    }

    #[test]
    fn mutating_tools_not_read_only() {
        assert_eq!(annotations("create_todo").unwrap().read_only_hint, Some(false));
        assert_eq!(annotations("create_reminder").unwrap().read_only_hint, Some(false));
        assert_eq!(annotations("schedule_action").unwrap().read_only_hint, Some(false));
    }

    #[test]
    fn unknown_tool_has_no_annotations() {
        assert!(annotations("does_not_exist").is_none());
    }
}
