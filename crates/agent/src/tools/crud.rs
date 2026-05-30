//! Tool implementations: create_todo, create_note, create_event, create_reminder.

use serde_json::Value;
use grumps_calendar::{NewEvent, EventSource};
use super::ToolContext;

pub async fn create_todo(ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    let title = args.get("title").and_then(|v| v.as_str())
        .ok_or_else(|| worker::Error::RustError("create_todo: missing 'title'".into()))?;
    let assignee = args.get("assignee").and_then(|v| v.as_str());
    let deadline = args.get("deadline").and_then(|v| v.as_str());
    let priority = args.get("priority").and_then(|v| v.as_i64()).unwrap_or(3) as i32;
    let tags: Vec<String> = args.get("tags")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let id = ctx.db.create_todo_simple(
        title,
        assignee,
        deadline,
        priority,
        tags,
        Some(ctx.member_id),
    ).await?;

    Ok(serde_json::json!({ "id": id, "created": true, "title": title }))
}

pub async fn create_note(ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    let content = args.get("content").and_then(|v| v.as_str())
        .ok_or_else(|| worker::Error::RustError("create_note: missing 'content'".into()))?;
    let title = args.get("title").and_then(|v| v.as_str());

    let id = ctx.db.create_note_simple(title, content, Some(ctx.member_id)).await?;

    Ok(serde_json::json!({ "id": id, "created": true }))
}

pub async fn create_event(ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    let title = args.get("title").and_then(|v| v.as_str())
        .ok_or_else(|| worker::Error::RustError("create_event: missing 'title'".into()))?;

    let starts_at_str = args.get("starts_at").and_then(|v| v.as_str())
        .ok_or_else(|| worker::Error::RustError("create_event: missing 'starts_at'".into()))?;

    let tz: chrono_tz::Tz = ctx.timezone.parse().unwrap_or(chrono_tz::UTC);
    let starts_at = super::parse_user_datetime(starts_at_str, &tz)
        .ok_or_else(|| worker::Error::RustError("create_event: invalid 'starts_at' datetime".into()))?;

    let ends_at = args.get("ends_at").and_then(|v| v.as_str())
        .and_then(|s| super::parse_user_datetime(s, &tz));

    let description = args.get("description").and_then(|v| v.as_str()).map(String::from);
    let location = args.get("location").and_then(|v| v.as_str()).map(String::from);
    let all_day = args.get("all_day").and_then(|v| v.as_bool()).unwrap_or(false);
    let color = args.get("color").and_then(|v| v.as_str()).map(String::from);

    let event = NewEvent {
        title: title.to_string(),
        description,
        starts_at,
        ends_at,
        all_day,
        location,
        recurrence: None,
        attendees: vec![],
        color,
        source: EventSource::Agent,
        related_todo_id: None,
        created_by: Some(ctx.member_id.to_string()),
    };

    let id = ctx.db.create_event(&event).await?;
    Ok(serde_json::json!({ "id": id, "created": true, "title": title }))
}

pub async fn create_reminder(ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    // Field names match the tool schema: `text` (message) + `trigger_at` (time).
    let text = args.get("text").and_then(|v| v.as_str())
        .ok_or_else(|| worker::Error::RustError("create_reminder: missing 'text'".into()))?;
    let trigger_at_str = args.get("trigger_at").and_then(|v| v.as_str())
        .ok_or_else(|| worker::Error::RustError("create_reminder: missing 'trigger_at'".into()))?;
    let recurrence = args.get("recurrence").and_then(|v| v.as_str());
    let target = args.get("target_member").and_then(|v| v.as_str())
        .unwrap_or(ctx.member_id);

    // Interpret the model's local wall-clock time in the workspace timezone and
    // store it as UTC (RFC3339 with Z) so the cron comparison stays correct.
    let tz: chrono_tz::Tz = ctx.timezone.parse().unwrap_or(chrono_tz::UTC);
    let remind_at = super::parse_user_datetime(trigger_at_str, &tz)
        .ok_or_else(|| worker::Error::RustError("create_reminder: invalid 'trigger_at' datetime".into()))?;
    let remind_at_str = remind_at.format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let id = ctx.db.insert_reminder(text, &remind_at_str, recurrence, target, ctx.member_id).await?;
    Ok(serde_json::json!({ "id": id, "created": true, "text": text, "trigger_at": remind_at_str }))
}
