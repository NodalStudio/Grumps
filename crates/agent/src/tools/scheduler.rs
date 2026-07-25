//! Tool implementations: schedule_action, cancel_scheduled, update_scheduled.

use super::{ScheduledAckCard, ToolContext};
use grumps_scheduler::{ActionType, NewScheduledAction};
use serde_json::Value;

pub async fn schedule_action(ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or_else(|| worker::Error::RustError("schedule_action: missing 'title'".into()))?;
    let trigger_at_str = args
        .get("trigger_at")
        .and_then(|v| v.as_str())
        .ok_or_else(|| worker::Error::RustError("schedule_action: missing 'trigger_at'".into()))?;
    let tz: chrono_tz::Tz = ctx.timezone.parse().unwrap_or(chrono_tz::UTC);
    let trigger_at = super::parse_user_datetime(trigger_at_str, &tz).ok_or_else(|| {
        worker::Error::RustError("schedule_action: invalid 'trigger_at' datetime".into())
    })?;

    let action_type_str = args
        .get("action_type")
        .and_then(|v| v.as_str())
        .unwrap_or("reminder");
    let action_type: ActionType =
        serde_json::from_value(Value::String(action_type_str.to_string()))
            .unwrap_or(ActionType::Reminder);

    let recurrence = args
        .get("recurrence")
        .and_then(|v| v.as_str())
        .map(String::from);
    let payload = args
        .get("payload")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let action = NewScheduledAction {
        action_type,
        title: title.to_string(),
        trigger_at,
        recurrence,
        condition: None,
        payload,
        target_chat: Some("group".to_string()),
        created_by: Some(ctx.member_id.to_string()),
    };

    let id = ctx.db.create_scheduled_action(&action).await?;
    Ok(
        serde_json::json!({ "id": id, "created": true, "title": title, "trigger_at": trigger_at_str }),
    )
}

/// Cancel (delete) a scheduled action created by `schedule_action` or
/// `create_reminder` — both land as rows in `scheduled_actions`. The DO alarm
/// is re-armed by the caller after the agent run returns (see
/// `route_via_agent` / `scheduler_executor::execute_action`), not here.
pub async fn cancel_scheduled(ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    let id = args
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| worker::Error::RustError("cancel_scheduled: missing 'id'".into()))?;
    let existing = ctx.db.get_scheduled_action(id).await?.ok_or_else(|| {
        worker::Error::RustError(format!("cancel_scheduled: no scheduled action '{id}'"))
    })?;

    let deleted = ctx.db.delete_scheduled_action(id).await?;
    if deleted {
        ctx.cancelled_scheduled.borrow_mut().push(ScheduledAckCard {
            id: id.to_string(),
            title: existing.title,
        });
    }
    Ok(serde_json::json!({ "id": id, "cancelled": deleted }))
}

/// Partial update — an omitted field keeps the existing value; `recurrence`
/// additionally treats an explicit empty string as "clear the recurrence"
/// (same sentinel convention the todo deadline update uses).
pub async fn update_scheduled(ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    let id = args
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| worker::Error::RustError("update_scheduled: missing 'id'".into()))?;
    let existing = ctx.db.get_scheduled_action(id).await?.ok_or_else(|| {
        worker::Error::RustError(format!("update_scheduled: no scheduled action '{id}'"))
    })?;

    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or(&existing.title);

    let trigger_at = match args.get("trigger_at").and_then(|v| v.as_str()) {
        Some(s) => {
            let tz: chrono_tz::Tz = ctx.timezone.parse().unwrap_or(chrono_tz::UTC);
            super::parse_user_datetime(s, &tz).ok_or_else(|| {
                worker::Error::RustError("update_scheduled: invalid 'trigger_at'".into())
            })?
        }
        None => existing.trigger_at,
    };

    let recurrence: Option<String> = match args.get("recurrence").and_then(|v| v.as_str()) {
        Some("") => None,
        Some(s) => Some(s.to_string()),
        None => existing.recurrence.clone(),
    };

    let trigger_at_iso = trigger_at.to_rfc3339();
    let updated = ctx
        .db
        .update_scheduled_action(
            id,
            title,
            &trigger_at_iso,
            recurrence.as_deref(),
            &existing.payload,
        )
        .await?;
    if updated {
        ctx.updated_scheduled.borrow_mut().push(ScheduledAckCard {
            id: id.to_string(),
            title: title.to_string(),
        });
    }
    Ok(
        serde_json::json!({ "id": id, "updated": updated, "title": title, "trigger_at": trigger_at_iso }),
    )
}
