//! Tool implementation: list_calendar.
//! Aggregates events + todos-with-deadline + reminders + scheduled actions in a date range.

use serde_json::Value;
use super::ToolContext;

pub async fn list_calendar(ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    let from = args.get("from").and_then(|v| v.as_str())
        .ok_or_else(|| worker::Error::RustError("list_calendar: missing 'from'".into()))?;
    let to = args.get("to").and_then(|v| v.as_str())
        .ok_or_else(|| worker::Error::RustError("list_calendar: missing 'to'".into()))?;

    let events = ctx.db.list_events_in_range(from, to).await?;
    let todos = ctx.db.list_todos_with_deadline(from, to).await?;
    let reminders = ctx.db.list_reminders_in_range(from, to).await?;
    let scheduled = ctx.db.list_scheduled_in_range(from, to).await?;

    Ok(serde_json::json!({
        "events": events.iter().map(|e| serde_json::json!({
            "id": e.id,
            "title": e.title,
            "starts_at": e.starts_at.to_rfc3339(),
            "ends_at": e.ends_at.map(|d| d.to_rfc3339()),
            "all_day": e.all_day,
            "location": e.location,
        })).collect::<Vec<_>>(),
        "todos": todos,
        "reminders": reminders,
        "scheduled": scheduled.iter().map(|a| serde_json::json!({
            "id": a.id,
            "title": a.title,
            "trigger_at": a.trigger_at.to_rfc3339(),
            "action_type": serde_json::to_value(&a.action_type).unwrap_or_default(),
        })).collect::<Vec<_>>(),
    }))
}
