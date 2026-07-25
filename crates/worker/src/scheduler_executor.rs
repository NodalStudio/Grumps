//! Dispatch scheduled actions when their alarm fires.
//! See spec § 7.4.
//!
//! Handles: reminder, event_notify, recap, follow_up, agent_task.

use crate::d1_rest::D1RestClient;
use crate::db::{get_index_db, lookup_workspace_by_slug, WorkspaceDb, WorkspaceMetaRow};
use grumps_scheduler::recurrence;
use grumps_scheduler::{ActionType, ScheduledAction};
use worker::*;

pub async fn execute_action(env: &Env, ws_slug: &str, action: &ScheduledAction) -> Result<()> {
    let index = get_index_db(env)?;
    let ws = lookup_workspace_by_slug(&index, ws_slug)
        .await?
        .ok_or_else(|| Error::RustError(format!("workspace not found: {ws_slug}")))?;
    let client = D1RestClient::from_env(env)?;
    let db = WorkspaceDb::new(&client, ws.d1_database_id.clone());

    // Condition gate. The DB-backed ConditionContext is still stubbed (it cannot
    // run async D1 queries from the sync `ConditionContext` trait), so evaluating
    // a real condition used to ALWAYS fail → reschedule forever → silently
    // mark_failed after 7 days, i.e. the action never fired. Until conditions are
    // genuinely evaluable (needs an async trait or pre-fetched context), a present
    // condition no longer blocks execution — the action fires at trigger_at. We
    // still validate the JSON shape so malformed payloads are surfaced.
    if let Some(cond_value) = &action.condition {
        if let Err(e) = serde_json::from_value::<grumps_scheduler::Condition>(cond_value.clone()) {
            console_log!(
                "execute_action {}: malformed condition JSON ({e}) — ignoring, executing",
                action.id
            );
        } else {
            console_log!(
                "execute_action {}: condition present but not yet evaluated (stub) — executing",
                action.id
            );
        }
    }

    let send_result = match action.action_type {
        ActionType::Reminder => execute_reminder(env, &ws, &db, action).await,
        ActionType::EventNotify => execute_event_notify(env, &ws, &db, action).await,
        ActionType::Recap => execute_recap(env, &ws, &db, action).await,
        ActionType::FollowUp | ActionType::AgentTask => {
            let instruction = action
                .payload
                .get("instruction")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| {
                    action
                        .payload
                        .get("prompt")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&action.title)
                })
                .to_string();

            let sink = crate::agent_sink::WorkerMessagingSink {
                env,
                ws_slug: ws_slug.to_string(),
                ws_db: &db,
            };

            let language = if ws.locale.is_empty() {
                "en".to_string()
            } else {
                ws.locale.clone()
            };
            let timezone = db
                .get_setting("timezone")
                .await
                .ok()
                .flatten()
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
                created_todos: std::cell::RefCell::new(Vec::new()),
            };

            let run_result = grumps_agent::loop_::run_oneshot(&ctx, &instruction).await;
            // Same seam as the chat path (handler::route_via_agent): the model's
            // own reply is deliberately terse about anything create_todo made
            // (see prompt.rs RULES), so render the real task card here from
            // what the tool call recorded on ctx.
            let created_todos = ctx.drain_created_todos();
            match run_result {
                Ok(result) => {
                    console_log!(
                        "agent_task/follow_up executed: {} turns, {} tokens",
                        result.turns,
                        result.total_tokens
                    );
                    if !created_todos.is_empty() {
                        let locale = grumps_i18n::Locale::from_code(&ctx.language);
                        let tz = grumps_core::timeutil::tz_or_utc(&ctx.timezone);
                        let today = grumps_core::timeutil::today_in_tz(tz);
                        // Same batch rule as handle_add_todos/route_via_agent:
                        // hint + link at most once, on the first card.
                        let (show_hint, show_link) = crate::handler::card_chrome(&db, tz).await;
                        for (idx, c) in created_todos.into_iter().enumerate() {
                            let (deadline_display, _, _) = crate::handler::deadline_display_info(
                                locale,
                                c.deadline.as_deref(),
                                today,
                            );
                            let card = grumps_messaging::formatter::task_card(
                                c.seq_num,
                                &c.title,
                                c.assignee.as_deref(),
                                deadline_display.as_deref(),
                                c.priority,
                                &c.tags,
                                false, // create_todo (agent tool) doesn't support recurrence yet
                                show_hint && idx == 0,
                                if show_link && idx == 0 {
                                    Some(ws_slug)
                                } else {
                                    None
                                },
                            );
                            let _ = send_to_group(env, &ws, &db, &card, Some(&c.id)).await;
                        }
                    }
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
                    &db.get_setting("timezone")
                        .await
                        .ok()
                        .flatten()
                        .unwrap_or_default(),
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
            let err_msg = e.to_string();
            console_error!(
                "scheduled action send failed: workspace={} action_id={} action_type={:?} recurring={} error={}",
                ws_slug,
                action.id,
                action.action_type,
                action.recurrence.is_some(),
                err_msg
            );
            // Recurring actions must survive a transient failure: record the
            // error but advance to the next occurrence and keep firing
            // (status stays 'pending'). One-shot actions — or a recurrence
            // that has genuinely run out of future occurrences — still end
            // up 'failed', now with the loud log above instead of silence.
            let tz = grumps_core::timeutil::tz_or_utc(
                &db.get_setting("timezone")
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or_default(),
            );
            match recurrence::next_occurrence_after_failure(
                action.recurrence.as_deref(),
                action.trigger_at,
                tz,
            ) {
                Some(next) => {
                    db.reschedule_action_after_failure(&action.id, &next.to_rfc3339(), &err_msg)
                        .await?;
                }
                None => {
                    db.mark_action_failed(&action.id, &err_msg).await?;
                }
            }
        }
    }
    Ok(())
}

async fn execute_reminder(
    env: &Env,
    ws: &WorkspaceMetaRow,
    db: &WorkspaceDb<'_>,
    action: &ScheduledAction,
) -> Result<()> {
    let text = action
        .payload
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or(&action.title);
    let loc = grumps_i18n::Locale::from_code(&ws.locale);
    let body = grumps_i18n::t(loc, "agent.reminder.fire", &[("text", text)]);
    // If the reminder is linked to a todo, carry it so a "done" reply completes
    // that todo.
    let todo_id = action.payload.get("todo_id").and_then(|v| v.as_str());
    send_to_group(env, ws, db, &body, todo_id).await
}

async fn execute_event_notify(
    env: &Env,
    ws: &WorkspaceMetaRow,
    db: &WorkspaceDb<'_>,
    action: &ScheduledAction,
) -> Result<()> {
    let event_id = action
        .payload
        .get("event_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::RustError("event_notify missing event_id".into()))?;
    let lead = action
        .payload
        .get("lead_minutes")
        .and_then(|v| v.as_i64())
        .unwrap_or(15);
    let event = db
        .get_event(event_id)
        .await?
        .ok_or_else(|| Error::RustError(format!("event not found: {event_id}")))?;
    let loc = grumps_i18n::Locale::from_code(&ws.locale);
    let location = match event.location.as_deref() {
        Some(l) if !l.is_empty() => l.to_string(),
        _ => grumps_i18n::t(loc, "agent.event.location_unset", &[]),
    };
    let lead_str = lead.to_string();
    let body = grumps_i18n::t(
        loc,
        "agent.event.notify",
        &[
            ("lead", lead_str.as_str()),
            ("title", event.title.as_str()),
            ("location", location.as_str()),
        ],
    );
    send_to_group(env, ws, db, &body, None).await
}

async fn execute_recap(
    env: &Env,
    ws: &WorkspaceMetaRow,
    db: &WorkspaceDb<'_>,
    _action: &ScheduledAction,
) -> Result<()> {
    // Build the recap from live workspace data, same as the weekly cron path.
    let tz = db
        .get_setting("timezone")
        .await
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "UTC".to_string());
    let data = db.get_recap_data(&tz).await?;
    // Nothing worth reporting → stay silent rather than send an empty recap.
    if data.open == 0 && data.done_week == 0 && data.new_notes == 0 {
        return Ok(());
    }
    // Dedup against the weekly cron recap (same key shape) so a workspace with
    // both an automatic Monday recap and a scheduled recap can't get two.
    let kv = env.kv("KV")?;
    let recap_key = format!(
        "recap:{}:{}",
        ws.slug,
        grumps_core::timeutil::tz_today_str(grumps_core::timeutil::tz_or_utc(&tz))
    );
    if kv.get(&recap_key).text().await?.is_some() {
        return Ok(());
    }
    let high_prio: Vec<(i64, String, Option<String>, Option<String>)> = data
        .high_priority
        .iter()
        .map(|t| {
            (
                t.seq_num,
                t.title.clone(),
                t.assigned_name.clone(),
                t.deadline.clone(),
            )
        })
        .collect();
    let locale = if ws.locale.is_empty() {
        "en".to_string()
    } else {
        ws.locale.clone()
    };
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
    send_to_group(env, ws, db, &body, None).await?;
    // Mark today's recap as sent so the cron path (and any other scheduled
    // recap) skips it for the rest of the local day.
    kv.put(&recap_key, "1")?
        .expiration_ttl(86400)
        .execute()
        .await?;
    Ok(())
}

async fn send_to_group(
    env: &Env,
    ws: &WorkspaceMetaRow,
    db: &WorkspaceDb<'_>,
    body: &str,
    todo_id: Option<&str>,
) -> Result<()> {
    use grumps_messaging::adapter::OutboundMessage;
    let out = OutboundMessage::text(body.to_string());
    // Record the sent message so a later reply to it is recognized as a reply
    // to Grumps (is_bot_message) and resolves to the linked todo, if any. Only
    // Telegram returns an id today; other platforms yield None and are skipped.
    if let Some(sent_id) = crate::messaging_dispatch::send_to_workspace(env, &ws.slug, &out).await?
    {
        let _ = db.track_bot_message(&sent_id, todo_id).await;
    }
    Ok(())
}

// The stubbed ConditionContext was removed: it could not evaluate conditions
// (the trait is sync, D1 is async) and made conditional actions never fire.
// See the condition gate in execute_action. Real evaluation needs an async
// ConditionContext (or a pre-fetched context) — tracked as a follow-up.
