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

    // Condition gate. If the action carries a condition, evaluate it against
    // live workspace state before firing. The `ConditionContext` trait is sync
    // and D1 is async, so we pre-fetch exactly the datum the condition names
    // (one todo's status / one member's last activity), build a sync context,
    // and evaluate. A condition that is NOT met means "skip the side effect this
    // time" — we still advance any recurrence below, so a recurring action keeps
    // its schedule. A malformed condition fails closed (skip, don't fire).
    let condition_met = match &action.condition {
        None => true,
        Some(cond_value) => match serde_json::from_value::<grumps_scheduler::Condition>(cond_value.clone()) {
            Ok(cond) => {
                let cctx = prefetch_condition_context(&db, &cond).await?;
                let met = grumps_scheduler::evaluate(&cond, &cctx);
                if !met {
                    console_log!("execute_action {}: condition not met — skipping side effect", action.id);
                }
                met
            }
            Err(e) => {
                console_log!("execute_action {}: malformed condition JSON ({e}) — skipping (fail closed)", action.id);
                false
            }
        },
    };

    let send_result = if !condition_met {
        // Treat as a successful no-op: recurrence still advances, one-shots are
        // marked done. The action simply produced no message this cycle.
        Ok(())
    } else { match action.action_type {
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
                // Scheduled tasks run with full authority (already user-scheduled).
                autonomy: grumps_agent::tools::Autonomy::Reactive,
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
    } };

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
    let tz = db.get_setting("timezone").await.ok().flatten()
        .filter(|s| !s.is_empty()).unwrap_or_else(|| "UTC".to_string());
    let data = db.get_recap_data(&tz).await?;
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
    let out = OutboundMessage { text: body.to_string(), reply_to: None, reply_markup: None };
    crate::messaging_dispatch::send_to_workspace(env, &ws.slug, &out).await
}

/// A sync [`grumps_scheduler::ConditionContext`] holding values pre-fetched from
/// D1. Each condition references at most one todo and one member, so we resolve
/// those up front (async) and hand the evaluator a plain snapshot.
struct PrefetchedConditionContext {
    last_active: Option<chrono::DateTime<chrono::Utc>>,
    todo_status: Option<String>,
    now: chrono::DateTime<chrono::Utc>,
}

impl grumps_scheduler::ConditionContext for PrefetchedConditionContext {
    fn last_active_at(&self, _member_id: &str) -> Option<chrono::DateTime<chrono::Utc>> {
        self.last_active
    }
    fn todo_status_now(&self, _todo_id: &str) -> Option<String> {
        self.todo_status.clone()
    }
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        self.now
    }
}

/// Resolve the (single) datum a condition names so it can be evaluated sync.
async fn prefetch_condition_context(
    db: &WorkspaceDb<'_>,
    cond: &grumps_scheduler::Condition,
) -> Result<PrefetchedConditionContext> {
    use grumps_scheduler::Condition;
    let mut last_active = None;
    let mut todo_status = None;
    match cond {
        Condition::TodoStatus { todo_id, .. } => {
            todo_status = db.get_todo_status(todo_id).await?;
        }
        Condition::MemberActiveAfter { member_id, .. }
        | Condition::MemberInactiveFor { member_id, .. } => {
            last_active = db.get_member_last_seen(member_id).await?;
        }
    }
    Ok(PrefetchedConditionContext { last_active, todo_status, now: chrono::Utc::now() })
}
