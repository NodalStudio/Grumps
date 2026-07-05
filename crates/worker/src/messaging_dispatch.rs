//! Thin helper to send a message to a workspace's chat group.
//! Resolves the platform from the Index DB, then routes through the
//! corresponding adapter in `grumps_messaging` so request construction
//! lives in one place.

use crate::db::{get_index_db, lookup_platform_channel};
use grumps_messaging::adapter::{MessagingPlatform, OutboundMessage};
use grumps_messaging::telegram::TelegramAdapter;
use worker::*;

/// Send a message to a workspace's chat group. Returns the platform message id
/// of the sent message when the platform reports it (Telegram), so callers can
/// record it via `track_bot_message` for reply detection. `None` for platforms
/// that don't (yet) surface an id.
pub async fn send_to_workspace(
    env: &Env,
    ws_slug: &str,
    out: &OutboundMessage,
) -> Result<Option<String>> {
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

async fn send_via_telegram(
    env: &Env,
    chat_id: &str,
    out: &OutboundMessage,
) -> Result<Option<String>> {
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
    // Best-effort: read back the sent message id for reply tracking.
    let sent_id = resp
        .json::<serde_json::Value>()
        .await
        .ok()
        .and_then(|json| {
            json.pointer("/result/message_id")
                .and_then(|v| v.as_i64())
                .map(|id| id.to_string())
        });
    Ok(sent_id)
}
