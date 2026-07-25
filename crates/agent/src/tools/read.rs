//! Tool implementations: list_todos, list_notes.
//! Read-only counterparts to crud.rs's create_todo/create_note — without these
//! the agent has no way to see the group's existing todos/notes and will
//! hallucinate an answer instead of checking the DB.

use super::ToolContext;
use serde_json::Value;

pub async fn list_todos(ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    let status = match args.get("status").and_then(|v| v.as_str()) {
        Some(s) if matches!(s, "open" | "done" | "all") => s,
        _ => "open",
    };

    let todos = ctx.db.list_todos(status).await?;
    Ok(serde_json::json!({ "todos": todos }))
}

pub async fn list_notes(ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    // Clamp to [1, 50] so a bad model-emitted value can't request an unbounded scan.
    let limit = args
        .get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(20)
        .clamp(1, 50);

    let notes = ctx.db.list_notes(limit).await?;
    Ok(serde_json::json!({ "notes": notes }))
}
