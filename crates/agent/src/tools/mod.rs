//! Tool registry + dispatch.

pub mod schemas;
pub mod memory;
pub mod rag;
pub mod rag_pipeline;
pub mod crud;
pub mod scheduler;
pub mod calendar;
pub mod web;
pub mod chat;

use worker::Env;
use crate::router::MessagingSink;
use crate::db::AgentDb;

/// Context passed to each tool handler.
pub struct ToolContext<'a> {
    pub env: &'a Env,
    pub workspace_slug: &'a str,
    pub member_id: &'a str,
    pub sink: &'a dyn MessagingSink,
    pub db: &'a dyn AgentDb,
}

/// Dispatch a tool call to its handler.
/// Returns a JSON value that becomes the tool_result content for the next Sonnet turn.
pub async fn dispatch(
    ctx: &ToolContext<'_>,
    tool_name: &str,
    args: serde_json::Value,
) -> worker::Result<serde_json::Value> {
    match tool_name {
        "query_memory"       => memory::query_memory(ctx, args).await,
        "save_memory"        => memory::save_memory(ctx, args).await,
        "query_chat_history" => rag::query_chat_history(ctx, args).await,
        "create_todo"        => crud::create_todo(ctx, args).await,
        "create_note"        => crud::create_note(ctx, args).await,
        "create_event"       => crud::create_event(ctx, args).await,
        "create_reminder"    => crud::create_reminder(ctx, args).await,
        "schedule_action"    => scheduler::schedule_action(ctx, args).await,
        "list_calendar"      => calendar::list_calendar(ctx, args).await,
        "web_search"         => web::web_search(ctx, args).await,
        "send_message"       => chat::send_message(ctx, args).await,
        other => Err(worker::Error::RustError(format!("unknown tool: {other}"))),
    }
}
