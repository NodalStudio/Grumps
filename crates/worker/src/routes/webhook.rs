use worker::*;
use grumps_messaging::adapter::MessagingPlatform;
use grumps_messaging::whatsapp::WhatsAppAdapter;
use grumps_nlu::parser;
use crate::{db, d1_rest::D1RestClient, provisioning, handler, llm_client::LlmClient};

pub async fn handle_verify(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let wa = build_adapter(&ctx)?;
    let params: std::collections::HashMap<String, String> = req.url()?.query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string())).collect();
    match wa.handle_verification_challenge(&params) {
        Ok(c) => Response::ok(c),
        Err(e) => Response::error(format!("{}", e), 403),
    }
}

pub async fn handle_incoming(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let wa = build_adapter(&ctx)?;
    let body = req.bytes().await?;

    // 1. HMAC verify
    if let Some(sig) = req.headers().get("X-Hub-Signature-256")? {
        if wa.verify_signature(&body, &sig).is_err() {
            return Response::error("Bad signature", 403);
        }
    }

    // 2. Parse webhook
    let inbound = match wa.parse_webhook(&body) {
        Ok(Some(m)) => m,
        Ok(None) => return Response::ok("ok"),
        Err(e) => { console_log!("Parse error: {:?}", e); return Response::ok("ok"); }
    };

    // 3. Dedup via KV
    let kv = ctx.kv("KV")?;
    let dedup_key = format!("msg:{}", inbound.message_id);
    if kv.get(&dedup_key).text().await?.is_some() {
        return Response::ok("ok");
    }
    kv.put(&dedup_key, "1")?.expiration_ttl(86400).execute().await?;

    // 4. Resolve or provision workspace
    let index_db = db::get_index_db(&ctx.env)?;
    let d1_client = D1RestClient::from_env(&ctx.env)?;

    let workspace = match db::lookup_workspace(&index_db, "whatsapp", &inbound.channel_id).await? {
        Some(ws) => ws,
        None => {
            let (slug, db_id) = provisioning::provision_workspace(
                &d1_client, &index_db, "whatsapp", &inbound.channel_id,
            ).await?;
            db::WorkspaceMetaRow { slug, d1_database_id: db_id, name: None, plan: "free".into() }
        }
    };

    let ws_db = db::WorkspaceDb::new(&d1_client, workspace.d1_database_id.clone());

    // 5. Upsert member (first = admin)
    let (member_id, is_first) = ws_db.upsert_member(&inbound.sender_id, &inbound.sender_name).await?;

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

    let parse_result = parser::parse(
        text,
        inbound.is_mention_to_bot,
        inbound.is_direct_message,
        is_reply_to_bot,
        inbound.quoted_message_id.is_some(),
    );

    // 7. Handle
    let result = handler::handle_message(
        parse_result,
        &inbound.message_id,
        inbound.quoted_message_id.as_deref(),
        inbound.quoted_message_text.as_deref(),
        &inbound.sender_name,
        &ws_db,
        &member_id,
        &workspace.slug,
        llm_client.as_ref(),
    ).await?;

    // 8. Send each message + track bot message IDs
    for msg in &result.messages {
        let (url, body) = wa.build_send_request(&inbound.sender_id, msg)
            .map_err(|e| worker::Error::RustError(format!("{:?}", e)))?;

        let mut headers = Headers::new();
        headers.set("Authorization", &format!("Bearer {}", wa.access_token))?;
        headers.set("Content-Type", "application/json")?;

        let mut init = RequestInit::new();
        init.with_method(Method::Post).with_headers(headers).with_body(Some(body.into()));

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
