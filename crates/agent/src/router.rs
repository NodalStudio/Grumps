//! Cascade router : regex fast-path → Gemini classifier → CRUD direct OR Sonnet agent loop.
//! See spec § 8.1.

use crate::db::AgentDb;
use crate::llm::gemini;
use crate::tools::{
    self, CreatedTodoCard, NoteAckCard, ScheduledAckCard, TodoAckCard, ToolContext,
};
use serde::Serialize;
use std::cell::RefCell;
use worker::*;

#[derive(Debug, Clone, Serialize)]
pub struct RouteResult {
    /// Final text the agent wants to send to the group (None if it stayed silent).
    pub final_text: Option<String>,
    /// Whether the agent already sent the message via sink (true) or expects the
    /// caller to send `final_text` (false).
    pub already_sent: bool,
    /// Todos created via `create_todo` during this run (also receives the
    /// spawned next-occurrence card from `complete_todo`). The caller renders
    /// a real task card for each (see `formatter::task_card`) instead of
    /// relying on the model's own prose — see `ToolContext::created_todos`.
    pub created_todos: Vec<CreatedTodoCard>,
    /// Todos completed via `complete_todo` during this run.
    pub completed_todos: Vec<TodoAckCard>,
    /// Todos mutated via `update_todo` during this run.
    pub updated_todos: Vec<TodoAckCard>,
    /// Todos removed via `delete_todo` during this run.
    pub deleted_todos: Vec<TodoAckCard>,
    /// Notes mutated via `update_note` during this run.
    pub updated_notes: Vec<NoteAckCard>,
    /// Notes removed via `delete_note` during this run.
    pub deleted_notes: Vec<NoteAckCard>,
    /// Scheduled actions removed via `cancel_scheduled` during this run.
    pub cancelled_scheduled: Vec<ScheduledAckCard>,
    /// Scheduled actions mutated via `update_scheduled` during this run.
    pub updated_scheduled: Vec<ScheduledAckCard>,
}

/// Drain every mutation accumulator on `ctx` into a `RouteResult`. All three
/// `route_message` exit points set `already_sent: true` (the agent always
/// sends its own reply via the sink before returning), so that's the only
/// field this doesn't fill in.
fn drain_route_result(ctx: &ToolContext<'_>, final_text: Option<String>) -> RouteResult {
    RouteResult {
        final_text,
        already_sent: true,
        created_todos: ctx.drain_created_todos(),
        completed_todos: ctx.drain_completed_todos(),
        updated_todos: ctx.drain_updated_todos(),
        deleted_todos: ctx.drain_deleted_todos(),
        updated_notes: ctx.drain_updated_notes(),
        deleted_notes: ctx.drain_deleted_notes(),
        cancelled_scheduled: ctx.drain_cancelled_scheduled(),
        updated_scheduled: ctx.drain_updated_scheduled(),
    }
}

#[async_trait::async_trait(?Send)]
pub trait MessagingSink {
    async fn send(&self, text: &str) -> Result<()>;
}

pub async fn route_message<'a>(
    env: &'a Env,
    ws_slug: &'a str,
    member_id: &'a str,
    text: &'a str,
    has_active_session: bool,
    sink: &'a dyn MessagingSink,
    db: &'a dyn AgentDb,
    locale: &'a str,
) -> Result<RouteResult> {
    // Locale is resolved by the caller from the canonical source (the workspace
    // member/meta locale in the index DB) — not a workspace-D1 setting.
    let language = if locale.is_empty() {
        "en".to_string()
    } else {
        locale.to_string()
    };
    let timezone = {
        let t = db.get_setting("timezone").await.unwrap_or_default();
        if t.is_empty() {
            "UTC".to_string()
        } else {
            t
        }
    };
    let ctx = ToolContext {
        env,
        workspace_slug: ws_slug,
        member_id,
        sink,
        db,
        language,
        timezone,
        created_todos: RefCell::new(Vec::new()),
        completed_todos: RefCell::new(Vec::new()),
        updated_todos: RefCell::new(Vec::new()),
        deleted_todos: RefCell::new(Vec::new()),
        updated_notes: RefCell::new(Vec::new()),
        deleted_notes: RefCell::new(Vec::new()),
        cancelled_scheduled: RefCell::new(Vec::new()),
        updated_scheduled: RefCell::new(Vec::new()),
    };

    // 1. If there's an active session, go straight to agent loop (multi-turn context).
    if has_active_session {
        let result = crate::loop_::run_loop(&ctx, text).await?;
        return Ok(drain_route_result(&ctx, result.final_text));
    }

    // 2. Otherwise, classify via Gemini Flash.
    let members_short = db.list_active_members().await.unwrap_or_default();
    let member_names: Vec<String> = members_short
        .iter()
        .map(|m| m.display_name.clone().unwrap_or_default())
        .collect();
    let classified = gemini::classify_intent_with_telemetry(
        env,
        db,
        "classify",
        Some(member_id),
        text,
        &member_names,
    )
    .await?;

    // 3. If high-confidence simple intent, dispatch CRUD directly.
    if !classified.is_complex() && classified.is_high_confidence() {
        match tools::dispatch(&ctx, &classified.intent, classified.args.clone()).await {
            Ok(result) => {
                // Format a short confirmation reply
                let msg = format_crud_confirmation(locale, &classified.intent, &result);
                sink.send(&msg).await?;
                return Ok(drain_route_result(&ctx, Some(msg)));
            }
            Err(e) => {
                console_log!(
                    "CRUD direct failed for intent {}, escalating to Sonnet: {e}",
                    classified.intent
                );
                // Fall through to agent loop
            }
        }
    }

    // 4. Otherwise, escalate to the full Sonnet agent loop.
    let result = crate::loop_::run_loop(&ctx, text).await?;
    Ok(drain_route_result(&ctx, result.final_text))
}

fn format_crud_confirmation(locale: &str, intent: &str, result: &serde_json::Value) -> String {
    use grumps_i18n::{t, Locale};
    let loc = Locale::from_code(locale);
    match intent {
        "create_todo" => {
            // The task card sent right after this (see route_via_agent) already
            // carries seq_num/title/assignee/deadline — show the human-facing
            // seq here, not the internal UUID `id` (which is what this used to
            // interpolate, and would have rendered as "#3f9a2b1c-...").
            let seq = result
                .get("seq_num")
                .and_then(|v| v.as_i64())
                .map(|n| n.to_string())
                .unwrap_or_else(|| "?".to_string());
            t(loc, "crud.todo_created", &[("id", &seq)])
        }
        "create_note" => t(loc, "crud.note_created", &[]),
        "create_event" => t(loc, "crud.event_created", &[]),
        "create_reminder" => t(loc, "crud.reminder_scheduled", &[]),
        "list_todos" => t(loc, "crud.list_todos", &[]),
        _ => t(loc, "crud.action_done", &[("intent", intent)]),
    }
}
