//! Thin helper to send a message to a workspace's chat group.
//! Resolves the platform from the Index DB, then routes through the
//! corresponding adapter in `grumps_messaging` so request construction
//! lives in one place.

use crate::db::{get_index_db, lookup_platform_channel};
use grumps_messaging::adapter::{is_send_ok, truncate_body, MessagingPlatform, OutboundMessage};
use grumps_messaging::telegram::TelegramAdapter;
use grumps_messaging::waha::WahaAdapter;
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
        "telegram" => send_via_telegram(env, ws_slug, &channel_id, out).await,
        // DB platform string stays "whatsapp" (WAHA is transport only — see
        // the route module docs); this is the WAHA REST send path.
        "whatsapp" => send_via_waha(env, ws_slug, &channel_id, out).await,
        "discord" => Err(Error::RustError(
            "send_to_workspace: Discord not wired yet — use the webhook path".into(),
        )),
        other => Err(Error::RustError(format!("unknown platform: {other}"))),
    }
}

async fn send_via_telegram(
    env: &Env,
    ws_slug: &str,
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
    let json = read_and_log_send_result(ws_slug, "telegram", &mut resp).await;
    let sent_id = json
        .pointer("/result/message_id")
        .and_then(|v| v.as_i64())
        .map(|id| id.to_string());
    Ok(sent_id)
}

async fn send_via_waha(
    env: &Env,
    ws_slug: &str,
    chat_id: &str,
    out: &OutboundMessage,
) -> Result<Option<String>> {
    let adapter = WahaAdapter::new(
        env.secret("WAHA_URL")?.to_string(),
        env.var("WAHA_SESSION")
            .map(|v| v.to_string())
            .unwrap_or_else(|_| "default".into()),
        env.secret("WAHA_API_KEY")?.to_string(),
        env.secret("WAHA_WEBHOOK_HMAC")?.to_string(),
    );
    let (url, body) = adapter
        .build_send_request(chat_id, out)
        .map_err(|e| Error::RustError(format!("{:?}", e)))?;
    let headers = Headers::new();
    headers.set("X-Api-Key", &adapter.api_key)?;
    headers.set("Content-Type", "application/json")?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(body.into()));
    let req = Request::new_with_init(&url, &init)?;
    let mut resp = Fetch::Request(req).send().await?;
    let json = read_and_log_send_result(ws_slug, "whatsapp", &mut resp).await;
    let sent_id = json
        .pointer("/key/id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Ok(sent_id)
}

/// Read back an outbound-send response and log a failure via
/// `console_error!` (workspace slug, platform, status, truncated body) so a
/// dropped reply leaves a server-side trace instead of vanishing silently.
/// Returns the parsed JSON body (`Value::Null` if the body isn't valid JSON)
/// so callers can still best-effort extract a sent-message id.
async fn read_and_log_send_result(
    ws_slug: &str,
    platform: &str,
    resp: &mut Response,
) -> serde_json::Value {
    let status = resp.status_code();
    let text = resp.text().await.unwrap_or_default();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    if !is_send_ok(platform, status, &json) {
        console_error!(
            "send failed: workspace={} platform={} status={} body={}",
            ws_slug,
            platform,
            status,
            truncate_body(&text, 300)
        );
    }
    json
}
