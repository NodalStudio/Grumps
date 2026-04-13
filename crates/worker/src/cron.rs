// crates/worker/src/cron.rs
use worker::*;
use crate::{db, d1_rest::D1RestClient};
use grumps_messaging::adapter::{MessagingPlatform, OutboundMessage};

/// Called by Cloudflare Cron Trigger. Iterates over all workspaces, fires due reminders.
pub async fn handle_cron(env: &Env) -> Result<()> {
    let index_db = db::get_index_db(env)?;
    let d1_client = D1RestClient::from_env(env)?;

    #[derive(serde::Deserialize)]
    struct WsRow {
        slug: String,
        d1_database_id: String,
        platform_channel_id: String,
    }

    let ws_results = index_db
        .prepare("SELECT slug, d1_database_id, platform_channel_id FROM workspaces_meta")
        .all()
        .await?;
    let workspaces: Vec<WsRow> = ws_results.results()?;

    let wa = crate::routes::webhook::build_adapter_from_env(env)?;

    for ws in &workspaces {
        let ws_db = db::WorkspaceDb::new(&d1_client, ws.d1_database_id.clone());
        let due = ws_db.get_due_reminders().await?;

        for reminder in &due {
            let target_text = reminder
                .target_member
                .as_ref()
                .map(|t| format!("@{} ", t))
                .unwrap_or_default();
            let msg_text = format!("\u{23f0} Reminder: {}{}", target_text, reminder.title);

            let msg = OutboundMessage { text: msg_text, reply_to: None };
            let (url, body) = wa
                .build_send_request(&ws.platform_channel_id, &msg)
                .map_err(|e| Error::RustError(format!("{:?}", e)))?;

            let mut headers = Headers::new();
            headers.set("Authorization", &format!("Bearer {}", wa.access_token))?;
            headers.set("Content-Type", "application/json")?;

            let mut init = RequestInit::new();
            init.with_method(Method::Post)
                .with_headers(headers)
                .with_body(Some(wasm_bindgen::JsValue::from_str(&body)));
            let req = Request::new_with_init(&url, &init)?;
            let _ = Fetch::Request(req).send().await;

            ws_db
                .fire_reminder(&reminder.id, reminder.recurrence.as_deref())
                .await?;

            console_log!(
                "Fired reminder {} in workspace {}",
                reminder.id,
                ws.slug
            );
        }
    }

    Ok(())
}
