//! Thin helper to send a message to a workspace's chat group.
//! Resolves the platform from the Index DB, then routes through the
//! corresponding adapter in `grumps_messaging` so request construction
//! lives in one place.

use worker::*;
use grumps_messaging::adapter::{MessagingPlatform, OutboundMessage};
use grumps_messaging::telegram::TelegramAdapter;
use crate::db::{get_index_db, lookup_platform_channel};

pub async fn send_to_workspace(env: &Env, ws_slug: &str, out: &OutboundMessage) -> Result<()> {
    let index = get_index_db(env)?;
    let (platform, channel_id) = lookup_platform_channel(&index, ws_slug)
        .await?
        .ok_or_else(|| Error::RustError(format!("workspace not found: {ws_slug}")))?;
    match platform.as_str() {
        "telegram" => send_via_telegram(env, &channel_id, out).await,
        "whatsapp" => Err(Error::RustError(
            "send_to_workspace: WhatsApp not wired yet — use the webhook path".into(),
        )),
        "discord" => Err(Error::RustError(
            "send_to_workspace: Discord not wired yet — use the webhook path".into(),
        )),
        other => Err(Error::RustError(format!("unknown platform: {other}"))),
    }
}

/// Like [`send_to_workspace`] but returns the platform message id of the sent
/// message when available (Telegram). Used for interactive proposals whose
/// inline keyboard we may later edit/clear. WhatsApp/Discord don't support
/// inline buttons yet, so they fall back to a plain send and return `None`.
pub async fn send_to_workspace_with_markup(env: &Env, ws_slug: &str, out: &OutboundMessage) -> Result<Option<String>> {
    let index = get_index_db(env)?;
    let (platform, channel_id) = lookup_platform_channel(&index, ws_slug)
        .await?
        .ok_or_else(|| Error::RustError(format!("workspace not found: {ws_slug}")))?;
    match platform.as_str() {
        "telegram" => send_via_telegram_returning_id(env, &channel_id, out).await,
        // TODO(buttons): map reply_markup to WhatsApp interactive reply buttons /
        // Discord message components. For now these platforms aren't wired for
        // sending at all, so delegate to the erroring plain path.
        _ => send_to_workspace(env, ws_slug, out).await.map(|_| None),
    }
}

async fn send_via_telegram(env: &Env, chat_id: &str, out: &OutboundMessage) -> Result<()> {
    send_via_telegram_returning_id(env, chat_id, out).await.map(|_| ())
}

async fn send_via_telegram_returning_id(env: &Env, chat_id: &str, out: &OutboundMessage) -> Result<Option<String>> {
    let adapter = TelegramAdapter::new(
        env.secret("TG_BOT_TOKEN")?.to_string(),
        env.var("TG_BOT_USERNAME")?.to_string(),
        env.secret("TG_WEBHOOK_SECRET")?.to_string(),
    );
    let (url, body) = adapter
        .build_send_request(chat_id, out)
        .map_err(|e| Error::RustError(format!("{:?}", e)))?;
    let mut headers = Headers::new();
    headers.set("Content-Type", "application/json")?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(body.into()));
    let req = Request::new_with_init(&url, &init)?;
    let mut resp = Fetch::Request(req).send().await?;
    // Parse `{ ok, result: { message_id } }`; treat any parse miss as "no id".
    let json: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
    let id = json
        .get("result")
        .and_then(|r| r.get("message_id"))
        .and_then(|m| m.as_i64())
        .map(|id| id.to_string());
    Ok(id)
}
