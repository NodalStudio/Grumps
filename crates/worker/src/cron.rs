// crates/worker/src/cron.rs
use crate::{d1_rest::D1RestClient, db};
use grumps_messaging::adapter::{MessagingPlatform, OutboundMessage};
use worker::*;

/// Called by Cloudflare Cron Trigger. Iterates over all workspaces, fires due reminders and recaps.
pub async fn handle_cron(env: &Env) -> Result<()> {
    let index_db = db::get_index_db(env)?;
    let d1_client = D1RestClient::from_env(env)?;

    let wa = crate::routes::webhook::build_adapter_from_env(env)?;

    // Reminders are no longer fired here. They are scheduled_actions fired by
    // each workspace's Durable Object alarm (tz-correct recurrence). Legacy
    // `reminders` rows were migrated to scheduled_actions by migration 0005.

    // Recaps fire on the *local* Monday. The cron runs once globally, but each
    // workspace has its own timezone, so the Monday check happens per-workspace
    // inside the helper rather than gating on UTC Monday here.
    if let Err(e) = check_and_send_recaps(env, &index_db, &d1_client, &wa).await {
        console_log!("Recap error: {:?}", e);
    }

    Ok(())
}

async fn check_and_send_recaps(
    env: &Env,
    index_db: &D1Database,
    d1_client: &D1RestClient,
    wa: &grumps_messaging::whatsapp::WhatsAppAdapter,
) -> Result<()> {
    #[derive(serde::Deserialize)]
    struct WsRow {
        slug: String,
        d1_database_id: String,
        platform_channel_id: String,
        #[serde(default)]
        locale: String,
    }

    let ws_results = index_db
        .prepare("SELECT slug, d1_database_id, platform_channel_id, locale FROM workspaces_meta")
        .all()
        .await?;
    let workspaces: Vec<WsRow> = ws_results.results()?;

    let kv = env.kv("KV")?;

    for ws in &workspaces {
        let ws_db = db::WorkspaceDb::new(d1_client, ws.d1_database_id.clone());

        // Recap fires on the workspace's *local* Monday.
        let tz = grumps_core::timeutil::tz_or_utc(
            &ws_db
                .get_setting("timezone")
                .await
                .ok()
                .flatten()
                .unwrap_or_default(),
        );
        if grumps_core::timeutil::tz_weekday(tz) != chrono::Weekday::Mon {
            continue;
        }

        // Check settings: is recap enabled? (default: enabled)
        let enabled = match ws_db.get_setting("recap_enabled").await {
            Ok(Some(v)) => v == "true",
            _ => true,
        };
        if !enabled {
            continue;
        }

        // KV dedup: at most one recap per local day per workspace.
        let recap_key = format!(
            "recap:{}:{}",
            ws.slug,
            grumps_core::timeutil::tz_today_str(tz)
        );
        if kv.get(&recap_key).text().await?.is_some() {
            continue;
        }

        // Get recap data
        let data = ws_db.get_recap_data(tz.name()).await?;

        // Only send if there's something to report
        if data.open == 0 && data.done_week == 0 && data.new_notes == 0 {
            continue;
        }

        // Format message
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
        let text = grumps_messaging::formatter::recap_message(
            &ws.slug,
            data.open,
            data.assigned,
            data.done_week,
            &high_prio,
            data.new_notes,
            data.reminders,
            if ws.locale.is_empty() {
                "en"
            } else {
                &ws.locale
            },
        );

        // Send to WhatsApp group
        let msg = OutboundMessage {
            text,
            ..Default::default()
        };
        let (url, body) = wa
            .build_send_request(&ws.platform_channel_id, &msg)
            .map_err(|e| Error::RustError(format!("{:?}", e)))?;

        let headers = Headers::new();
        headers.set("Authorization", &format!("Bearer {}", wa.access_token))?;
        headers.set("Content-Type", "application/json")?;

        let mut init = RequestInit::new();
        init.with_method(Method::Post)
            .with_headers(headers)
            .with_body(Some(wasm_bindgen::JsValue::from_str(&body)));
        let req = Request::new_with_init(&url, &init)?;
        let _ = Fetch::Request(req).send().await;

        // Mark as sent (expires after 24h)
        kv.put(&recap_key, "1")?
            .expiration_ttl(86400)
            .execute()
            .await?;
        console_log!("Sent recap for workspace {}", ws.slug);
    }

    Ok(())
}
