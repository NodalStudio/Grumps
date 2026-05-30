//! Dispatch scheduled actions when their alarm fires.
//! See spec § 7.4.
//!
//! Handles: reminder, event_notify, recap, follow_up, agent_task.

use worker::*;
use grumps_scheduler::{ScheduledAction, ActionType};
use crate::db::{WorkspaceDb, get_index_db, lookup_workspace_by_slug, WorkspaceMetaRow};
use crate::d1_rest::D1RestClient;
use grumps_scheduler::recurrence;

pub async fn execute_action(env: &Env, ws_slug: &str, action: &ScheduledAction) -> Result<()> {
    let index = get_index_db(env)?;
    let ws = lookup_workspace_by_slug(&index, ws_slug).await?
        .ok_or_else(|| Error::RustError(format!("workspace not found: {ws_slug}")))?;
    let client = D1RestClient::from_env(env)?;
    let db = WorkspaceDb::new(&client, ws.d1_database_id.clone());

    // Evaluate condition if present
    if let Some(cond_value) = &action.condition {
        let condition: grumps_scheduler::Condition = match serde_json::from_value(cond_value.clone()) {
            Ok(c) => c,
            Err(e) => {
                console_log!("execute_action: bad condition JSON for {}: {e}, treating as fail-open (true)", action.id);
                // Bad JSON — mark failed and bail
                db.mark_action_failed(&action.id, &format!("bad condition JSON: {e}")).await?;
                return Ok(());
            }
        };
        let ctx = WorkerConditionContext { db: &db };
        if !grumps_scheduler::evaluate(&condition, &ctx) {
            // Condition not met — reschedule for later check or give up after 7d
            let recheck_in = action.payload.get("recheck_in_seconds")
                .and_then(|v| v.as_i64())
                .unwrap_or(3600);
            let next = action.trigger_at + chrono::Duration::seconds(recheck_in);
            let give_up_at = action.created_at + chrono::Duration::days(7);
            if next > give_up_at {
                console_log!("condition give-up for {}: 7d elapsed", action.id);
                db.mark_action_failed(&action.id, "condition never met within 7d").await?;
            } else {
                console_log!("condition not met for {}, recheck at {next}", action.id);
                db.reschedule_action(&action.id, &next.to_rfc3339()).await?;
            }
            return Ok(());
        }
    }

    let send_result = match action.action_type {
        ActionType::Reminder => execute_reminder(env, &ws, action).await,
        ActionType::EventNotify => execute_event_notify(env, &ws, &db, action).await,
        ActionType::Recap => execute_recap(env, &ws, &db, action).await,
        ActionType::FollowUp | ActionType::AgentTask => {
            let instruction = action.payload.get("instruction")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| {
                    action.payload.get("prompt")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&action.title)
                })
                .to_string();

            let sink = crate::agent_sink::WorkerMessagingSink {
                env,
                ws_slug: ws_slug.to_string(),
            };

            let language = if ws.locale.is_empty() { "en".to_string() } else { ws.locale.clone() };
            let timezone = db.get_setting("timezone").await.ok().flatten()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "UTC".to_string());
            let ctx = grumps_agent::tools::ToolContext {
                env,
                workspace_slug: ws_slug,
                member_id: "system",
                sink: &sink,
                db: &db,
                language,
                timezone,
            };

            match grumps_agent::loop_::run_oneshot(&ctx, &instruction).await {
                Ok(result) => {
                    console_log!(
                        "agent_task/follow_up executed: {} turns, {} tokens",
                        result.turns,
                        result.total_tokens
                    );
                    Ok(())
                }
                Err(e) => {
                    console_log!("agent_task/follow_up error: {e}");
                    Err(e)
                }
            }
        }
    };

    // Mark done (or re-schedule if recurrent)
    match send_result {
        Ok(()) => {
            if let Some(rrule) = &action.recurrence {
                let parsed = recurrence::parse(rrule)
                    .map_err(|e| Error::RustError(format!("bad rrule: {e}")))?;
                // Recurrence weekday/BYHOUR are evaluated in the workspace tz.
                let tz = grumps_core::timeutil::tz_or_utc(
                    &db.get_setting("timezone").await.ok().flatten().unwrap_or_default(),
                );
                if let Some(next) = recurrence::next_occurrence(&parsed, action.trigger_at, tz) {
                    db.reschedule_action(&action.id, &next.to_rfc3339()).await?;
                } else {
                    db.mark_action_done(&action.id).await?;
                }
            } else {
                db.mark_action_done(&action.id).await?;
            }
        }
        Err(e) => {
            db.mark_action_failed(&action.id, &e.to_string()).await?;
        }
    }
    Ok(())
}

async fn execute_reminder(env: &Env, ws: &WorkspaceMetaRow, action: &ScheduledAction) -> Result<()> {
    let text = action.payload.get("text").and_then(|v| v.as_str()).unwrap_or(&action.title);
    let body = format!("⏰ Rappel : {text}");
    send_to_group(env, ws, &body).await
}

async fn execute_event_notify(env: &Env, ws: &WorkspaceMetaRow, db: &WorkspaceDb<'_>, action: &ScheduledAction) -> Result<()> {
    let event_id = action.payload.get("event_id").and_then(|v| v.as_str())
        .ok_or_else(|| Error::RustError("event_notify missing event_id".into()))?;
    let lead = action.payload.get("lead_minutes").and_then(|v| v.as_i64()).unwrap_or(15);
    let event = db.get_event(event_id).await?
        .ok_or_else(|| Error::RustError(format!("event not found: {event_id}")))?;
    let body = format!("📅 Dans {lead}min : {} ({})", event.title,
        event.location.as_deref().unwrap_or("lieu non précisé"));
    send_to_group(env, ws, &body).await
}

async fn execute_recap(env: &Env, ws: &WorkspaceMetaRow, db: &WorkspaceDb<'_>, _action: &ScheduledAction) -> Result<()> {
    // Build the recap from live workspace data, same as the weekly cron path.
    let data = db.get_recap_data().await?;
    // Nothing worth reporting → stay silent rather than send an empty recap.
    if data.open == 0 && data.done_week == 0 && data.new_notes == 0 {
        return Ok(());
    }
    let high_prio: Vec<(i64, String, Option<String>, Option<String>)> = data.high_priority.iter()
        .map(|t| (t.seq_num, t.title.clone(), t.assigned_name.clone(), t.deadline.clone()))
        .collect();
    let locale = if ws.locale.is_empty() { "en".to_string() } else { ws.locale.clone() };
    let body = grumps_messaging::formatter::recap_message(
        &ws.slug,
        data.open,
        data.assigned,
        data.done_week,
        &high_prio,
        data.new_notes,
        data.reminders,
        &locale,
    );
    send_to_group(env, ws, &body).await
}

async fn send_to_group(env: &Env, ws: &WorkspaceMetaRow, body: &str) -> Result<()> {
    use grumps_messaging::adapter::OutboundMessage;
    let out = OutboundMessage { text: body.to_string(), reply_to: None };
    crate::messaging_dispatch::send_to_workspace(env, &ws.slug, &out).await
}

// =============================================
// ConditionContext (V1 stubs)
// =============================================

struct WorkerConditionContext<'a> {
    db: &'a WorkspaceDb<'a>,
}

impl<'a> grumps_scheduler::ConditionContext for WorkerConditionContext<'a> {
    fn count_messages_matching(&self, _since: chrono::DateTime<chrono::Utc>, _keywords: &[String]) -> i64 {
        // V1 stub: no chat-message table in D1 (RAG lives in Vectorize).
        // Return 0 → conservative (NoMessageMatching fires when count < min, so 0 → fires).
        0
    }
    fn last_active_at(&self, _member_id: &str) -> Option<chrono::DateTime<chrono::Utc>> {
        // V1 stub: would query members.last_seen_at
        None
    }
    fn todo_status_now(&self, _todo_id: &str) -> Option<String> {
        // V1 stub: would query todos.status
        None
    }
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now()
    }
}
