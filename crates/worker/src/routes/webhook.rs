use crate::{d1_rest::D1RestClient, db, handler, llm_client::LlmClient, provisioning};
use grumps_messaging::adapter::MessagingPlatform;
use grumps_messaging::whatsapp::WhatsAppAdapter;
use grumps_nlu::parser;
use worker::*;

pub async fn handle_verify(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let wa = build_adapter(&ctx)?;
    let params: std::collections::HashMap<String, String> = req
        .url()?
        .query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    match wa.handle_verification_challenge(&params) {
        Ok(c) => Response::ok(c),
        Err(e) => Response::error(format!("{}", e), 403),
    }
}

pub async fn handle_incoming(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let wa = build_adapter(&ctx)?;
    let body = req.bytes().await?;

    // 1. HMAC verify. Signature is mandatory: an attacker who omits the header
    // would otherwise skip verification and inject arbitrary webhook payloads.
    let sig = req
        .headers()
        .get("X-Hub-Signature-256")?
        .ok_or_else(|| Error::RustError("missing X-Hub-Signature-256 header".into()))?;
    if wa.verify_signature(&body, &sig).is_err() {
        return Response::error("Bad signature", 403);
    }

    // 2. Parse webhook
    let inbound = match wa.parse_webhook(&body) {
        Ok(Some(m)) => m,
        Ok(None) => return Response::ok("ok"),
        Err(e) => {
            console_log!("Parse error: {:?}", e);
            return Response::ok("ok");
        }
    };

    crate::observability::log_inbound(
        &crate::observability::request_id(&req),
        "whatsapp",
        &inbound.channel_id,
        &inbound.sender_id,
        &inbound.sender_name,
        &inbound.message_id,
        inbound.text.as_deref(),
        inbound.is_mention_to_bot,
        inbound.is_direct_message,
    );

    // 3. Dedup via KV
    let kv = ctx.kv("KV")?;
    let dedup_key = format!("msg:whatsapp:{}", inbound.message_id);
    if kv.get(&dedup_key).text().await?.is_some() {
        return Response::ok("ok");
    }
    kv.put(&dedup_key, "1")?
        .expiration_ttl(86400)
        .execute()
        .await?;

    // 4. Resolve or provision workspace
    let index_db = db::get_index_db(&ctx.env)?;
    let d1_client = D1RestClient::from_env(&ctx.env)?;

    let workspace = match db::lookup_workspace(&index_db, "whatsapp", &inbound.channel_id).await? {
        Some(ws) => ws,
        None => {
            let (slug, db_id) = provisioning::provision_workspace(
                &d1_client,
                &index_db,
                "whatsapp",
                &inbound.channel_id,
            )
            .await?;
            db::WorkspaceMetaRow {
                slug,
                d1_database_id: db_id,
                name: None,
                plan: "free".into(),
                locale: "en".into(),
            }
        }
    };

    let ws_db = db::WorkspaceDb::new(&d1_client, workspace.d1_database_id.clone());

    // 5. Upsert member (first = admin)
    let (member_id, is_first) = ws_db
        .upsert_member(&inbound.sender_id, &inbound.sender_name)
        .await?;

    // Register in Index DB
    let role = if is_first { "admin" } else { "member" };
    let _ = db::upsert_index_user(&index_db, &inbound.sender_id, &workspace.slug, role).await;

    // 5b. Create LLM client (optional — works without API keys)
    let llm_client = LlmClient::from_env(&ctx.env).ok();

    // 6. Parse message text
    let text = match &inbound.text {
        Some(t) => t.as_str(),
        None => return Response::ok("ok"),
    };

    let is_reply_to_bot = match &inbound.quoted_message_id {
        Some(qid) => ws_db.is_bot_message(qid).await.unwrap_or(false),
        None => false,
    };

    // Persist to the chat-history log (every message, even short ones) and keep
    // the row's UUIDv7 as the anchor for RAG context windows. Best-effort.
    let anchor_id = match ws_db
        .insert_message(
            "whatsapp",
            &inbound.message_id,
            &member_id,
            &inbound.sender_name,
            text,
            &inbound.timestamp.to_rfc3339(),
        )
        .await
    {
        Ok(id) => id,
        Err(e) => {
            worker::console_log!("message log error (whatsapp): {e}");
            String::new()
        }
    };

    // RAG ingest (best-effort, non-blocking on failure)
    {
        let meta = grumps_agent::tools::rag_pipeline::ChatVectorMetadata {
            workspace_slug: workspace.slug.to_string(),
            platform: "whatsapp".into(),
            sender_member_id: member_id.to_string(),
            sender_name: inbound.sender_name.to_string(),
            text: text.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            anchor_id: anchor_id.clone(),
        };
        if let Err(e) = grumps_agent::tools::rag_pipeline::ingest_message(&ctx.env, &meta).await {
            worker::console_log!("RAG ingest error (whatsapp): {e}");
        }
    }

    // Unified ambient classifier (auto-memory + proactive + quality-signal feedback)
    {
        let auto_memory = ws_db
            .get_setting("auto_memory")
            .await
            .ok()
            .flatten()
            .as_deref()
            == Some("true");
        let proactive = ws_db
            .get_setting("proactive_mode")
            .await
            .ok()
            .flatten()
            .as_deref()
            == Some("true");
        let feedback_disabled = ws_db
            .get_setting("quality_feedback_disabled")
            .await
            .ok()
            .flatten()
            .as_deref()
            == Some("true");
        let modes = grumps_agent::ambient::AmbientModes {
            auto_memory,
            proactive_mode: proactive,
            feedback_detection: !feedback_disabled,
        };
        if auto_memory || proactive || !feedback_disabled {
            let upper = text.trim_start().to_uppercase();
            let is_command = upper.starts_with("TODO:")
                || upper.starts_with("DONE:")
                || upper.starts_with("NOTE:")
                || upper.starts_with("REMIND:");
            if !is_command {
                let members_short = ws_db.get_members().await.unwrap_or_default();
                let member_names: Vec<String> = members_short
                    .iter()
                    .filter_map(|m| m.display_name.clone())
                    .collect();
                let pinned = ws_db.list_pinned_memory().await.unwrap_or_default();
                let pinned_summary: String = pinned
                    .iter()
                    .take(10)
                    .map(|m| format!("- {}", m.value))
                    .collect::<Vec<_>>()
                    .join("\n");
                let recent = ws_db
                    .list_recent_bot_actions(1800, 10)
                    .await
                    .unwrap_or_default();
                let analysis = grumps_agent::ambient::analyze_with_telemetry(
                    &ctx.env,
                    &ws_db,
                    Some(&member_id),
                    text,
                    &member_names,
                    &pinned_summary,
                    &recent,
                    &modes,
                )
                .await;
                let sink = crate::agent_sink::WorkerMessagingSink {
                    env: &ctx.env,
                    ws_slug: workspace.slug.clone(),
                };
                let _ = grumps_agent::ambient::apply_analysis(
                    &ctx.env,
                    &ws_db,
                    &sink,
                    &workspace.slug,
                    &member_id,
                    text,
                    &analysis,
                    &recent,
                )
                .await;
            }
        }
    }

    let parse_result = parser::parse(
        text,
        inbound.is_mention_to_bot,
        inbound.is_direct_message,
        is_reply_to_bot,
        inbound.quoted_message_id.is_some(),
    );

    // 7. Handle
    let result = handler::handle_message(
        Some(&ctx.env),
        text,
        parse_result,
        &inbound.message_id,
        inbound.quoted_message_id.as_deref(),
        inbound.quoted_message_text.as_deref(),
        &inbound.sender_name,
        &ws_db,
        &member_id,
        &workspace.slug,
        &workspace.locale,
        llm_client.as_ref(),
        &workspace.plan,
    )
    .await?;

    // 8. Send each message + track bot message IDs
    for msg in &result.messages {
        let (url, body) = wa
            .build_send_request(&inbound.sender_id, msg)
            .map_err(|e| worker::Error::RustError(format!("{:?}", e)))?;

        let headers = Headers::new();
        headers.set("Authorization", &format!("Bearer {}", wa.access_token))?;
        headers.set("Content-Type", "application/json")?;

        let mut init = RequestInit::new();
        init.with_method(Method::Post)
            .with_headers(headers)
            .with_body(Some(body.into()));

        let meta_req = Request::new_with_init(&url, &init)?;
        let mut meta_resp = Fetch::Request(meta_req).send().await?;

        // Track bot's sent message_id for reply detection
        if let Ok(json) = meta_resp.json::<serde_json::Value>().await {
            if let Some(sent_id) = json.pointer("/messages/0/id").and_then(|v| v.as_str()) {
                let _ = ws_db.track_bot_message(sent_id, None).await;
            }
        }
    }

    Response::ok("ok")
}

fn build_adapter(ctx: &RouteContext<()>) -> Result<WhatsAppAdapter> {
    Ok(WhatsAppAdapter::new(
        ctx.env.var("WA_PHONE_NUMBER_ID")?.to_string(),
        ctx.env.var("WA_VERIFY_TOKEN")?.to_string(),
        ctx.env.secret("WA_APP_SECRET")?.to_string(),
        ctx.env.secret("WA_ACCESS_TOKEN")?.to_string(),
    ))
}

/// Public version for use by other routes (auth).
pub fn build_adapter_from_env(env: &Env) -> Result<grumps_messaging::whatsapp::WhatsAppAdapter> {
    Ok(grumps_messaging::whatsapp::WhatsAppAdapter::new(
        env.var("WA_PHONE_NUMBER_ID")?.to_string(),
        env.var("WA_VERIFY_TOKEN")?.to_string(),
        env.secret("WA_APP_SECRET")?.to_string(),
        env.secret("WA_ACCESS_TOKEN")?.to_string(),
    ))
}
