//! Tool implementations: create_todo, create_note, create_event, create_reminder,
//! complete_todo, update_todo, delete_todo, update_note, delete_note.

use super::{CreatedTodoCard, NoteAckCard, TodoAckCard, ToolContext};
use crate::db::NoteBrief;
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
        recurring: false, // create_todo (agent tool) doesn't support recurrence
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

/// Pure extraction of the `seq` argument — split out from `resolve_todo` so
/// the "did the model send something we can even look up" step is unit
/// testable without a mock `AgentDb`.
fn seq_arg(args: &Value) -> Option<i64> {
    args.get("seq").and_then(|v| v.as_i64())
}

/// Pure extraction of the `id_or_title` argument — same rationale as `seq_arg`.
fn id_or_title_arg(args: &Value) -> Option<&str> {
    args.get("id_or_title").and_then(|v| v.as_str())
}

/// Resolve a model-emitted `seq` (integer, human-facing) to the row mutation
/// tools operate on. `Err` becomes a tool_result error the model sees (see
/// `loop_::run_loop`) — it can then call `list_todos` and retry with the
/// right seq, or tell the user the todo wasn't found.
async fn resolve_todo(
    ctx: &ToolContext<'_>,
    args: &Value,
    tool: &str,
) -> worker::Result<crate::db::TodoBrief> {
    let seq =
        seq_arg(args).ok_or_else(|| worker::Error::RustError(format!("{tool}: missing 'seq'")))?;
    ctx.db
        .get_todo_by_seq(seq)
        .await?
        .ok_or_else(|| worker::Error::RustError(format!("{tool}: no todo with seq #{seq}")))
}

/// Resolve `update_note`/`delete_note`'s `id_or_title` argument: an exact id
/// match first, then a normalized-title match (same `title_norm` lookup the
/// wikilink resolver uses) — lets the model reference a note it only knows
/// by title, not just ones it just fetched via `list_notes`.
async fn resolve_note(
    ctx: &ToolContext<'_>,
    args: &Value,
    tool: &str,
) -> worker::Result<NoteBrief> {
    let key = id_or_title_arg(args)
        .ok_or_else(|| worker::Error::RustError(format!("{tool}: missing 'id_or_title'")))?;
    if let Some(n) = ctx.db.get_note_by_id(key).await? {
        return Ok(n);
    }
    ctx.db
        .get_note_by_title(key)
        .await?
        .ok_or_else(|| worker::Error::RustError(format!("{tool}: no note found for '{key}'")))
}

pub async fn complete_todo(ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    let todo = resolve_todo(ctx, &args, "complete_todo").await?;
    if todo.status == "done" {
        return Err(worker::Error::RustError(format!(
            "complete_todo: todo #{} is already done",
            todo.seq_num
        )));
    }

    ctx.db.complete_todo(&todo.id, ctx.member_id).await?;
    ctx.completed_todos.borrow_mut().push(TodoAckCard {
        id: todo.id.clone(),
        seq_num: todo.seq_num,
        title: todo.title.clone(),
    });

    // Same recurrence-spawn semantics as the card-reply Done arm: a
    // completed recurring todo immediately spawns its next occurrence,
    // rendered as a real task card via the same created_todos mechanism
    // create_todo uses (see CreatedTodoCard::recurring).
    let mut next_occurrence_created = false;
    if let Some(rec) = todo.recurrence.as_deref().filter(|r| !r.is_empty()) {
        let tz: chrono_tz::Tz = ctx.timezone.parse().unwrap_or(chrono_tz::UTC);
        if let Ok(next) = ctx.db.create_next_recurrence(&todo, rec, tz).await {
            next_occurrence_created = true;
            ctx.created_todos.borrow_mut().push(CreatedTodoCard {
                id: next.id,
                seq_num: next.seq_num,
                title: next.title,
                assignee: next.assigned_name,
                deadline: next.deadline,
                priority: Priority::from_int(next.priority),
                tags: next.tags,
                recurring: true,
            });
        }
    }

    Ok(serde_json::json!({
        "id": todo.id,
        "seq_num": todo.seq_num,
        "completed": true,
        "next_occurrence_created": next_occurrence_created,
    }))
}

pub async fn update_todo(ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    let todo = resolve_todo(ctx, &args, "update_todo").await?;

    let title = args.get("title").and_then(|v| v.as_str());
    // Not "done" — that has its own recurrence-spawning tool (complete_todo).
    let status = args.get("status").and_then(|v| v.as_str());
    let priority = args
        .get("priority")
        .and_then(|v| v.as_i64())
        .map(|p| p as i32);
    let assignee = args.get("assignee").and_then(|v| v.as_str());
    if title.is_some() || status.is_some() || priority.is_some() || assignee.is_some() {
        // Same call the card-reply Edit/Reassign/ChangePriority/ChangeStatus
        // arms use — assigned_to and assigned_name both get the mention text,
        // mirroring the Reassign arm (chat replies only carry a display name).
        ctx.db
            .update_todo(&todo.id, title, status, priority, assignee, assignee)
            .await?;
    }

    if let Some(d) = args.get("deadline").and_then(|v| v.as_str()) {
        if d.is_empty() {
            ctx.db.set_todo_deadline(&todo.id, "").await?;
        } else {
            let tz: chrono_tz::Tz = ctx.timezone.parse().unwrap_or(chrono_tz::UTC);
            let parsed = super::parse_user_date(d, &tz).ok_or_else(|| {
                worker::Error::RustError("update_todo: invalid 'deadline'".into())
            })?;
            ctx.db.set_todo_deadline(&todo.id, &parsed).await?;
        }
    }

    if let Some(tags) = args.get("tags").and_then(|v| v.as_array()) {
        for tag in tags.iter().filter_map(|t| t.as_str()) {
            ctx.db.add_todo_tag(&todo.id, tag).await?;
        }
    }

    let new_title = title.unwrap_or(&todo.title).to_string();
    ctx.updated_todos.borrow_mut().push(TodoAckCard {
        id: todo.id.clone(),
        seq_num: todo.seq_num,
        title: new_title.clone(),
    });

    Ok(
        serde_json::json!({ "id": todo.id, "seq_num": todo.seq_num, "updated": true, "title": new_title }),
    )
}

pub async fn delete_todo(ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    let todo = resolve_todo(ctx, &args, "delete_todo").await?;
    ctx.db.delete_todo(&todo.id).await?;
    ctx.deleted_todos.borrow_mut().push(TodoAckCard {
        id: todo.id.clone(),
        seq_num: todo.seq_num,
        title: todo.title.clone(),
    });
    Ok(serde_json::json!({ "id": todo.id, "seq_num": todo.seq_num, "deleted": true }))
}

pub async fn update_note(ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    let note = resolve_note(ctx, &args, "update_note").await?;
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| note.title.as_deref().unwrap_or(""));
    let content = args
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or(&note.content);

    ctx.db
        .update_note(&note.id, title, content, ctx.member_id)
        .await?;
    ctx.updated_notes.borrow_mut().push(NoteAckCard {
        id: note.id.clone(),
        title: if title.is_empty() {
            None
        } else {
            Some(title.to_string())
        },
    });
    Ok(serde_json::json!({ "id": note.id, "updated": true }))
}

pub async fn delete_note(ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    let note = resolve_note(ctx, &args, "delete_note").await?;
    ctx.db.delete_note(&note.id).await?;
    ctx.deleted_notes.borrow_mut().push(NoteAckCard {
        id: note.id.clone(),
        title: note.title.clone(),
    });
    Ok(serde_json::json!({ "id": note.id, "deleted": true }))
}

#[cfg(test)]
mod tests {
    use super::{default_priority, id_or_title_arg, seq_arg};

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

    // ── seq_arg (complete_todo/update_todo/delete_todo resolution) ────────

    #[test]
    fn seq_arg_reads_integer() {
        assert_eq!(seq_arg(&serde_json::json!({ "seq": 3 })), Some(3));
    }

    #[test]
    fn seq_arg_missing_is_none() {
        assert_eq!(seq_arg(&serde_json::json!({})), None);
    }

    #[test]
    fn seq_arg_wrong_type_is_none() {
        // A stringly-typed "3" is not silently coerced — the model must send
        // a real JSON integer, matching the tool schema's `"type": "integer"`.
        assert_eq!(seq_arg(&serde_json::json!({ "seq": "3" })), None);
    }

    #[test]
    fn seq_arg_negative_and_zero_pass_through() {
        // Range/existence validation happens at the DB lookup (no todo has a
        // seq <= 0), not here — this is pure JSON extraction only.
        assert_eq!(seq_arg(&serde_json::json!({ "seq": 0 })), Some(0));
        assert_eq!(seq_arg(&serde_json::json!({ "seq": -1 })), Some(-1));
    }

    // ── id_or_title_arg (update_note/delete_note resolution) ──────────────

    #[test]
    fn id_or_title_arg_reads_string() {
        assert_eq!(
            id_or_title_arg(&serde_json::json!({ "id_or_title": "Wifi" })),
            Some("Wifi")
        );
    }

    #[test]
    fn id_or_title_arg_missing_is_none() {
        assert_eq!(id_or_title_arg(&serde_json::json!({})), None);
    }

    #[test]
    fn id_or_title_arg_wrong_type_is_none() {
        assert_eq!(
            id_or_title_arg(&serde_json::json!({ "id_or_title": 42 })),
            None
        );
    }
}
