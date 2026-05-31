//! Tool implementation: schedule_action.

use super::ToolContext;
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
