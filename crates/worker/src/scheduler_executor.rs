//! Dispatch scheduled actions when their alarm fires.
//! See spec § 7.4.
//!
//! Plan A scope : reminder, event_notify, recap.
//! follow_up + agent_task come in Plan B (require agent loop).

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

            let ctx = grumps_agent::tools::ToolContext {
                env,
                workspace_slug: ws_slug,
                member_id: "system",
                sink: &sink,
                db: &db,
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
                if let Some(next) = recurrence::next_occurrence(&parsed, action.trigger_at) {
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

async fn execute_recap(env: &Env, ws: &WorkspaceMetaRow, _db: &WorkspaceDb<'_>, _action: &ScheduledAction) -> Result<()> {
    // Reuse existing recap logic from worker/src/cron.rs if it exists.
    // For Plan A, we send a placeholder recap. Plan B will integrate real LLM-generated recap.
    let body = "📋 Recap hebdomadaire — placeholder (Plan B improves this with LLM-generated content).".to_string();
    send_to_group(env, ws, &body).await
}

async fn send_to_group(env: &Env, ws: &WorkspaceMetaRow, body: &str) -> Result<()> {
    use grumps_messaging::adapter::OutboundMessage;
    let out = OutboundMessage { text: body.to_string(), reply_to: None };
    crate::messaging_dispatch::send_to_workspace(env, &ws.slug, &out).await
}
