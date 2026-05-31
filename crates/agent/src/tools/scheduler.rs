//! Tool implementation: schedule_action.

use super::{args, parse_args, ToolContext};
use grumps_scheduler::NewScheduledAction;
use serde_json::Value;

pub async fn schedule_action(ctx: &ToolContext<'_>, raw: Value) -> worker::Result<Value> {
    let a: args::ScheduleActionArgs = parse_args(raw, "schedule_action")?;
    let tz: chrono_tz::Tz = ctx.timezone.parse().unwrap_or(chrono_tz::UTC);
    let trigger_at = super::parse_user_datetime(&a.trigger_at, &tz).ok_or_else(|| {
        worker::Error::RustError("schedule_action: invalid 'trigger_at' datetime".into())
    })?;

    let action = NewScheduledAction {
        action_type: a.action_type.into(),
        title: a.title.clone(),
        trigger_at,
        recurrence: a.recurrence,
        payload: Value::Object(a.payload),
        target_chat: Some("group".to_string()),
        created_by: Some(ctx.member_id.to_string()),
    };

    let id = ctx.db.create_scheduled_action(&action).await?;
    Ok(
        serde_json::json!({ "id": id, "created": true, "title": a.title, "trigger_at": a.trigger_at }),
    )
}
