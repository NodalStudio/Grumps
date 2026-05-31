//! Tool implementation: list_calendar.
//! Aggregates events + todos-with-deadline + scheduled actions in a date range.
//! Reminders are scheduled actions (`action_type == "reminder"`) — they appear
//! under `scheduled`, each carrying its `action_type`.

use super::{args, parse_args, ToolContext};
use serde_json::Value;

pub async fn list_calendar(ctx: &ToolContext<'_>, raw: Value) -> worker::Result<Value> {
    let a: args::ListCalendarArgs = parse_args(raw, "list_calendar")?;
    let (from, to) = (a.from.as_str(), a.to.as_str());

    let events = ctx.db.list_events_in_range(from, to).await?;
    let todos = ctx.db.list_todos_with_deadline(from, to).await?;
    let scheduled = ctx.db.list_scheduled_in_range(from, to).await?;

    Ok(serde_json::json!({
        "ok": true,
        "events": events.iter().map(|e| serde_json::json!({
            "id": e.id,
            "title": e.title,
            "starts_at": e.starts_at.to_rfc3339(),
            "ends_at": e.ends_at.map(|d| d.to_rfc3339()),
            "all_day": e.all_day,
            "location": e.location,
        })).collect::<Vec<_>>(),
        "todos": todos,
        "scheduled": scheduled.iter().map(|a| serde_json::json!({
            "id": a.id,
            "title": a.title,
            "trigger_at": a.trigger_at.to_rfc3339(),
            "action_type": serde_json::to_value(&a.action_type).unwrap_or_default(),
        })).collect::<Vec<_>>(),
    }))
}
