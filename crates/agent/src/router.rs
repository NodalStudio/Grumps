//! Cascade router : regex fast-path → Gemini classifier → CRUD direct OR Sonnet agent loop.
//! See spec § 8.1.

use crate::db::AgentDb;
use crate::llm::gemini;
use crate::tools::{self, ToolContext};
use serde::Serialize;
use worker::*;

#[derive(Debug, Clone, Serialize)]
pub struct RouteResult {
    /// Final text the agent wants to send to the group (None if it stayed silent).
    pub final_text: Option<String>,
    /// Whether the agent already sent the message via sink (true) or expects the
    /// caller to send `final_text` (false).
    pub already_sent: bool,
}

/// A tappable action button, platform-neutral. `id` becomes the platform's
/// callback payload (e.g. Telegram `callback_data`); `label` is the i18n'd text.
#[derive(Debug, Clone)]
pub struct ProposalButton {
    pub id: String,
    pub label: String,
}

#[async_trait::async_trait(?Send)]
pub trait MessagingSink {
    async fn send(&self, text: &str) -> Result<()>;

    /// Send a message carrying tappable action buttons. Returns the platform
    /// message id (so the caller can later edit/clear the keyboard) when the
    /// platform supports buttons; otherwise the default sends plain text and
    /// returns `None`. Platforms without inline buttons (WhatsApp/Discord today)
    /// inherit the default.
    async fn send_with_buttons(
        &self,
        text: &str,
        buttons: &[ProposalButton],
    ) -> Result<Option<String>>;
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
        autonomy: tools::Autonomy::Reactive,
    };

    // 1. If there's an active session, go straight to agent loop (multi-turn context).
    if has_active_session {
        let result = crate::loop_::run_loop(&ctx, text).await?;
        return Ok(RouteResult {
            final_text: result.final_text,
            already_sent: true,
        });
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
                let msg = format_crud_confirmation(&classified.intent, &result);
                sink.send(&msg).await?;
                return Ok(RouteResult {
                    final_text: Some(msg),
                    already_sent: true,
                });
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
    Ok(RouteResult {
        final_text: result.final_text,
        already_sent: true,
    })
}

fn format_crud_confirmation(intent: &str, result: &serde_json::Value) -> String {
    match intent {
        "create_todo" => {
            let id = result.get("id").and_then(|v| v.as_str()).unwrap_or("?");
            format!("✅ Todo créée (#{id})")
        }
        "create_note" => "📝 Note créée".to_string(),
        "create_event" => "📅 Event créé".to_string(),
        "create_reminder" => "⏰ Rappel programmé".to_string(),
        "list_todos" => "Voir la liste dans le workspace".to_string(),
        _ => format!("Action '{intent}' exécutée"),
    }
}
