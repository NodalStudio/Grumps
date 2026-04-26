//! Thin helper to send a message to a workspace's chat group.
//! Resolves platform from Index DB, builds adapter, sends.

use worker::*;
use grumps_messaging::adapter::OutboundMessage;
use crate::db::get_index_db;

pub async fn send_to_workspace(env: &Env, ws_slug: &str, out: &OutboundMessage) -> Result<()> {
    use serde::Deserialize;
    #[derive(Deserialize)]
    struct Row { platform: String, platform_channel_id: String }
    let index = get_index_db(env)?;
    let row: Option<Row> = index.prepare(
        "SELECT platform, platform_channel_id FROM workspaces_meta WHERE slug = ?1"
    ).bind(&[ws_slug.into()])?.first(None).await?;
    let row = row.ok_or_else(|| Error::RustError(format!("workspace not found: {ws_slug}")))?;
    match row.platform.as_str() {
        "telegram" => {
            let token = env.secret("TG_BOT_TOKEN")?.to_string();
            let body = serde_json::json!({
                "chat_id": row.platform_channel_id,
                "text": out.text,
                "parse_mode": "Markdown",
            });
            let url = format!("https://api.telegram.org/bot{token}/sendMessage");
            let headers = Headers::new();
            headers.set("content-type", "application/json")?;
            let req = Request::new_with_init(&url, RequestInit::new()
                .with_method(Method::Post)
                .with_headers(headers)
                .with_body(Some(serde_json::to_string(&body).unwrap().into())))?;
            Fetch::Request(req).send().await?;
            Ok(())
        }
        "whatsapp" => {
            console_log!("WA send not implemented in messaging_dispatch (Plan A) ; use existing handler path");
            Err(Error::RustError("WA send via messaging_dispatch not yet implemented".into()))
        }
        "discord" => {
            console_log!("Discord send not implemented in messaging_dispatch (Plan A)");
            Err(Error::RustError("Discord send via messaging_dispatch not yet implemented".into()))
        }
        other => Err(Error::RustError(format!("unknown platform: {other}"))),
    }
}
