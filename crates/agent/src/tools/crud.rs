//! Tool implementations: create_todo, create_note, create_event, create_reminder.

use super::{args, parse_args, ToolContext};
use grumps_calendar::{EventSource, NewEvent};
use serde_json::Value;

/// A civil date "YYYY-MM-DD" anchored at UTC midnight — the storage sentinel
/// for all-day events (the DB layer writes back the bare date).
fn civil_date_to_utc(date_str: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    use chrono::TimeZone;
    chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|ndt| chrono::Utc.from_utc_datetime(&ndt))
}

pub async fn create_todo(ctx: &ToolContext<'_>, raw: Value) -> worker::Result<Value> {
    let a: args::CreateTodoArgs = parse_args(raw, "create_todo")?;
    // A deadline is a civil date in the workspace tz — normalized to YYYY-MM-DD,
    // never converted to a UTC instant (which would shift the day).
    let tz: chrono_tz::Tz = ctx.timezone.parse().unwrap_or(chrono_tz::UTC);
    let deadline = a
        .deadline
        .as_deref()
        .and_then(|d| super::parse_user_date(d, &tz));
    let priority = a.priority.unwrap_or(3) as i32;
    let tags = a.tags.unwrap_or_default();

    let id = ctx
        .db
        .create_todo_simple(
            &a.title,
            a.assignee.as_deref(),
            deadline.as_deref(),
            priority,
            tags,
            Some(ctx.member_id),
        )
        .await?;

    Ok(serde_json::json!({ "id": id, "created": true, "title": a.title }))
}

pub async fn create_note(ctx: &ToolContext<'_>, raw: Value) -> worker::Result<Value> {
    let a: args::CreateNoteArgs = parse_args(raw, "create_note")?;
    let id = ctx
        .db
        .create_note_simple(a.title.as_deref(), &a.content, Some(ctx.member_id))
        .await?;
    Ok(serde_json::json!({ "id": id, "created": true }))
}

pub async fn create_event(ctx: &ToolContext<'_>, raw: Value) -> worker::Result<Value> {
    let a: args::CreateEventArgs = parse_args(raw, "create_event")?;

    let tz: chrono_tz::Tz = ctx.timezone.parse().unwrap_or(chrono_tz::UTC);
    let all_day = a.all_day.unwrap_or(false);
    let starts_at_str = a.starts_at.as_str();
    let ends_at_str = a.ends_at.as_deref();

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

    let event = NewEvent {
        title: a.title.clone(),
        description: a.description,
        starts_at,
        ends_at,
        all_day,
        location: a.location,
        recurrence: a.recurrence,
        attendees: a.attendees.unwrap_or_default(),
        color: a.color,
        source: EventSource::Agent,
        related_todo_id: None,
        created_by: Some(ctx.member_id.to_string()),
    };

    let id = ctx.db.create_event(&event).await?;
    Ok(serde_json::json!({ "id": id, "created": true, "title": a.title }))
}

pub async fn create_reminder(ctx: &ToolContext<'_>, raw: Value) -> worker::Result<Value> {
    use grumps_scheduler::{ActionType, NewScheduledAction};
    let a: args::CreateReminderArgs = parse_args(raw, "create_reminder")?;

    // Interpret the model's local wall-clock time in the workspace tz → UTC.
    let tz: chrono_tz::Tz = ctx.timezone.parse().unwrap_or(chrono_tz::UTC);
    let remind_at = super::parse_user_datetime(&a.trigger_at, &tz).ok_or_else(|| {
        worker::Error::RustError("create_reminder: invalid 'trigger_at' datetime".into())
    })?;
    let remind_at_str = remind_at.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let weekday = chrono::Datelike::weekday(&remind_at.with_timezone(&tz));
    let recurrence = a
        .recurrence
        .as_deref()
        .and_then(|r| grumps_scheduler::recurrence::text_to_rrule(r, weekday));
    let text = a.text;

    // A reminder is a scheduled_action fired by the workspace Durable Object
    // (unified with schedule_action). The DO alarm is (re)armed by the worker
    // after the agent loop returns.
    let action = NewScheduledAction {
        action_type: ActionType::Reminder,
        title: text.clone(),
        trigger_at: remind_at,
        recurrence,
        payload: serde_json::json!({ "text": text.clone() }),
        target_chat: Some("group".to_string()),
        created_by: Some(ctx.member_id.to_string()),
    };
    let id = ctx.db.create_scheduled_action(&action).await?;
    Ok(serde_json::json!({ "id": id, "created": true, "text": text, "trigger_at": remind_at_str }))
}
