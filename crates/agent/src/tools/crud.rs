//! Tool implementations: create_todo, create_note, create_event, create_reminder.

use super::{CreatedTodoCard, ToolContext};
use grumps_calendar::{EventSource, NewEvent};
use grumps_core::todo::Priority;
use serde_json::Value;

/// Priority defaults to Normal (2) — matching the deterministic `TODO: x`
/// parser path (see `grumps_core::todo::Priority::from_int`). Previously
/// defaulted to 3 (Low), which silently downgraded agent-created todos
/// relative to the same intent typed as a slash command.
fn default_priority(args: &Value) -> i32 {
    args.get("priority").and_then(|v| v.as_i64()).unwrap_or(2) as i32
}

/// A civil date "YYYY-MM-DD" anchored at UTC midnight — the storage sentinel
/// for all-day events (the DB layer writes back the bare date).
fn civil_date_to_utc(date_str: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    use chrono::TimeZone;
    chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|ndt| chrono::Utc.from_utc_datetime(&ndt))
}

pub async fn create_todo(ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| worker::Error::RustError("create_todo: missing 'title'".into()))?;
    let assignee = args.get("assignee").and_then(|v| v.as_str());
    // A deadline is a civil date in the workspace tz — normalized to YYYY-MM-DD,
    // never converted to a UTC instant (which would shift the day).
    let tz: chrono_tz::Tz = ctx.timezone.parse().unwrap_or(chrono_tz::UTC);
    let deadline = args
        .get("deadline")
        .and_then(|v| v.as_str())
        .and_then(|d| super::parse_user_date(d, &tz));
    let priority = default_priority(&args);
    let tags: Vec<String> = args
        .get("tags")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let (id, seq_num) = ctx
        .db
        .create_todo_simple(
            title,
            assignee,
            deadline.as_deref(),
            priority,
            tags.clone(),
            Some(ctx.member_id),
        )
        .await?;

    // Recorded so the caller (route_message / execute_action) can render the
    // real task card after the run — see ToolContext::created_todos.
    ctx.created_todos.borrow_mut().push(CreatedTodoCard {
        id: id.clone(),
        seq_num,
        title: title.to_string(),
        assignee: assignee.map(String::from),
        deadline: deadline.clone(),
        priority: Priority::from_int(priority),
        tags,
    });

    Ok(serde_json::json!({ "id": id, "seq_num": seq_num, "created": true, "title": title }))
}

pub async fn create_note(ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    let content = args
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| worker::Error::RustError("create_note: missing 'content'".into()))?;
    let title = args.get("title").and_then(|v| v.as_str());

    let id = ctx
        .db
        .create_note_simple(title, content, Some(ctx.member_id))
        .await?;

    Ok(serde_json::json!({ "id": id, "created": true }))
}

pub async fn create_event(ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| worker::Error::RustError("create_event: missing 'title'".into()))?;

    let starts_at_str = args
        .get("starts_at")
        .and_then(|v| v.as_str())
        .ok_or_else(|| worker::Error::RustError("create_event: missing 'starts_at'".into()))?;

    let tz: chrono_tz::Tz = ctx.timezone.parse().unwrap_or(chrono_tz::UTC);
    let all_day = args
        .get("all_day")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let ends_at_str = args.get("ends_at").and_then(|v| v.as_str());

    // All-day events are civil dates (no time, no tz shift): we anchor the date
    // at UTC midnight so the storage layer can write a bare "YYYY-MM-DD".
    // Timed events are instants converted from the local wall clock to UTC.
    let (starts_at, ends_at) = if all_day {
        let s = super::parse_user_date(starts_at_str, &tz)
            .and_then(|d| civil_date_to_utc(&d))
            .ok_or_else(|| {
                worker::Error::RustError("create_event: invalid 'starts_at' date".into())
            })?;
        let e = ends_at_str
            .and_then(|s| super::parse_user_date(s, &tz))
            .and_then(|d| civil_date_to_utc(&d));
        (s, e)
    } else {
        let s = super::parse_user_datetime(starts_at_str, &tz).ok_or_else(|| {
            worker::Error::RustError("create_event: invalid 'starts_at' datetime".into())
        })?;
        let e = ends_at_str.and_then(|s| super::parse_user_datetime(s, &tz));
        (s, e)
    };

    let description = args
        .get("description")
        .and_then(|v| v.as_str())
        .map(String::from);
    let location = args
        .get("location")
        .and_then(|v| v.as_str())
        .map(String::from);
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
    use grumps_scheduler::{ActionType, NewScheduledAction};
    // Field names match the tool schema: `text` (message) + `trigger_at` (time).
    let text = args
        .get("text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| worker::Error::RustError("create_reminder: missing 'text'".into()))?;
    let trigger_at_str = args
        .get("trigger_at")
        .and_then(|v| v.as_str())
        .ok_or_else(|| worker::Error::RustError("create_reminder: missing 'trigger_at'".into()))?;

    // Interpret the model's local wall-clock time in the workspace tz → UTC.
    let tz: chrono_tz::Tz = ctx.timezone.parse().unwrap_or(chrono_tz::UTC);
    let remind_at = super::parse_user_datetime(trigger_at_str, &tz).ok_or_else(|| {
        worker::Error::RustError("create_reminder: invalid 'trigger_at' datetime".into())
    })?;
    let remind_at_str = remind_at.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let weekday = chrono::Datelike::weekday(&remind_at.with_timezone(&tz));
    let recurrence = args
        .get("recurrence")
        .and_then(|v| v.as_str())
        .and_then(|r| grumps_scheduler::recurrence::text_to_rrule(r, weekday));

    // A reminder is a scheduled_action fired by the workspace Durable Object
    // (unified with schedule_action). The DO alarm is (re)armed by the worker
    // after the agent loop returns.
    let action = NewScheduledAction {
        action_type: ActionType::Reminder,
        title: text.to_string(),
        trigger_at: remind_at,
        recurrence,
        condition: None,
        payload: serde_json::json!({ "text": text }),
        target_chat: Some("group".to_string()),
        created_by: Some(ctx.member_id.to_string()),
    };
    let id = ctx.db.create_scheduled_action(&action).await?;
    Ok(serde_json::json!({ "id": id, "created": true, "text": text, "trigger_at": remind_at_str }))
}

#[cfg(test)]
mod tests {
    use super::default_priority;

    #[test]
    fn default_priority_is_normal_when_absent() {
        // Matches Priority::from_int's own default (Normal=2) — the
        // deterministic `TODO: x` parser path never defaults to Low.
        assert_eq!(default_priority(&serde_json::json!({})), 2);
    }

    #[test]
    fn default_priority_respects_explicit_value() {
        assert_eq!(default_priority(&serde_json::json!({ "priority": 1 })), 1);
        assert_eq!(default_priority(&serde_json::json!({ "priority": 3 })), 3);
    }
}
