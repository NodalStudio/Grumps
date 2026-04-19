use worker::*;
use grumps_messaging::adapter::MessagingPlatform;
use grumps_messaging::telegram::TelegramAdapter;
use grumps_nlu::parser;
use crate::{db, d1_rest::D1RestClient, provisioning, handler};

pub async fn handle_incoming(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let tg = build_adapter(&ctx)?;
    let body = req.bytes().await?;

    // Verify secret token
    if let Some(secret) = req.headers().get("X-Telegram-Bot-Api-Secret-Token")? {
        if tg.verify_signature(&body, &secret).is_err() {
            return Response::error("Bad secret", 403);
        }
    }

    // Check for bot-added-to-group event
    let raw: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|e| Error::RustError(e.to_string()))?;

    if let Some(member_update) = raw.get("my_chat_member") {
        let new_status = member_update.pointer("/new_chat_member/status")
            .and_then(|v| v.as_str()).unwrap_or("");
        let chat_id = member_update.pointer("/chat/id")
            .and_then(|v| v.as_i64()).map(|id| id.to_string()).unwrap_or_default();
        let chat_title = member_update.pointer("/chat/title")
            .and_then(|v| v.as_str()).unwrap_or("Group");

        if new_status == "member" || new_status == "administrator" {
            return handle_bot_added(&ctx, &tg, &chat_id, chat_title).await;
        }
    }

    let inbound = match tg.parse_webhook(&body) {
        Ok(Some(m)) => m,
        _ => return Response::ok("ok"),
    };

    // KV dedup
    let kv = ctx.kv("KV")?;
    let key = format!("msg:tg:{}", inbound.message_id);
    if kv.get(&key).text().await?.is_some() { return Response::ok("ok"); }
    kv.put(&key, "1")?.expiration_ttl(86400).execute().await?;

    // Resolve/provision workspace
    let index_db = db::get_index_db(&ctx.env)?;
    let d1_client = D1RestClient::from_env(&ctx.env)?;
    let workspace = match db::lookup_workspace(&index_db, "telegram", &inbound.channel_id).await? {
        Some(ws) => ws,
        None => {
            let (slug, db_id) = provisioning::provision_workspace(&d1_client, &index_db, "telegram", &inbound.channel_id).await?;
            db::WorkspaceMetaRow { slug, d1_database_id: db_id, name: None, plan: "free".into() }
        }
    };

    let ws_db = db::WorkspaceDb::new(&d1_client, workspace.d1_database_id.clone());
    let (member_id, is_first) = ws_db.upsert_member(&inbound.sender_id, &inbound.sender_name).await?;
    let role = if is_first { "admin" } else { "member" };
    let _ = db::upsert_index_user(&index_db, &inbound.sender_id, &workspace.slug, role).await;

    let text = match &inbound.text { Some(t) => t.as_str(), None => return Response::ok("ok") };

    // Strip bot mention and normalize to @grumps so existing NLU parser works unchanged
    let clean_text = text.replace(&format!("@{}", tg.bot_username), "@grumps")
        .replace(&format!("@{}", tg.bot_username.to_lowercase()), "@grumps");

    let is_reply_to_bot = match &inbound.quoted_message_id {
        Some(qid) => ws_db.is_bot_message(qid).await.unwrap_or(false),
        None => false,
    };

    // RAG ingest (best-effort, non-blocking on failure)
    {
        let meta = grumps_agent::tools::rag_pipeline::ChatVectorMetadata {
            workspace_slug: workspace.slug.to_string(),
            platform: "telegram".into(),
            sender_member_id: member_id.to_string(),
            sender_name: inbound.sender_name.to_string(),
            text: text.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        if let Err(e) = grumps_agent::tools::rag_pipeline::ingest_message(&ctx.env, &meta).await {
            worker::console_log!("RAG ingest error (telegram): {e}");
        }
    }

    let parse_result = parser::parse(&clean_text, inbound.is_mention_to_bot, inbound.is_direct_message, is_reply_to_bot, inbound.quoted_message_id.is_some());

    // LLM client (optional)
    let llm = crate::llm_client::LlmClient::from_env(&ctx.env).ok();

    let result = handler::handle_message(
        Some(&ctx.env),
        &clean_text,
        parse_result, &inbound.message_id,
        inbound.quoted_message_id.as_deref(), inbound.quoted_message_text.as_deref(),
        &inbound.sender_name, &ws_db, &member_id, &workspace.slug,
        llm.as_ref(), &workspace.plan,
    ).await?;

    // Send responses
    for msg in &result.messages {
        let (url, body) = tg.build_send_request(&inbound.channel_id, msg)
            .map_err(|e| Error::RustError(format!("{:?}", e)))?;
        let mut headers = Headers::new();
        headers.set("Content-Type", "application/json")?;
        let mut init = RequestInit::new();
        init.with_method(Method::Post).with_headers(headers).with_body(Some(body.into()));
        let req = Request::new_with_init(&url, &init)?;
        let mut resp = Fetch::Request(req).send().await?;
        // Track bot message
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if let Some(msg_id) = json.pointer("/result/message_id").and_then(|v| v.as_i64()) {
                let _ = ws_db.track_bot_message(&msg_id.to_string(), None).await;
            }
        }
    }

    Response::ok("ok")
}

async fn handle_bot_added(
    ctx: &RouteContext<()>,
    tg: &TelegramAdapter,
    chat_id: &str,
    chat_title: &str,
) -> Result<Response> {
    let index_db = db::get_index_db(&ctx.env)?;
    let d1_client = D1RestClient::from_env(&ctx.env)?;

    // Provision workspace
    let (slug, _db_id) = provisioning::provision_workspace(
        &d1_client, &index_db, "telegram", chat_id,
    ).await?;

    // Set group description (may fail if bot isn't admin — that's ok)
    let description = format!("Grumps workspace: grumps.io/w/{}\nGets it done. No small talk.", slug);
    let (desc_url, desc_body) = tg.build_set_description_request(chat_id, &description)
        .map_err(|e| Error::RustError(format!("{:?}", e)))?;
    {
        let mut headers = Headers::new();
        headers.set("Content-Type", "application/json")?;
        let mut init = RequestInit::new();
        init.with_method(Method::Post).with_headers(headers).with_body(Some(desc_body.into()));
        let req = Request::new_with_init(&desc_url, &init)?;
        let _ = Fetch::Request(req).send().await;
    }

    // Send welcome message
    let welcome = format!(
        "📋 *Grumps* is here.\n\n\
        Your workspace: grumps.io/w/{}\n\n\
        Quick start:\n\
        • `TODO:` + list to add tasks\n\
        • `DONE:` + list to complete them\n\
        • `NOTE:` to pin info\n\
        • `@{} help` for all commands\n\n\
        Gets it done. No small talk.",
        slug, tg.bot_username
    );
    let _ = chat_title; // available if needed for future use

    let msg = grumps_messaging::adapter::OutboundMessage { text: welcome, reply_to: None };
    let (url, body) = tg.build_send_request(chat_id, &msg)
        .map_err(|e| Error::RustError(format!("{:?}", e)))?;
    {
        let mut headers = Headers::new();
        headers.set("Content-Type", "application/json")?;
        let mut init = RequestInit::new();
        init.with_method(Method::Post).with_headers(headers).with_body(Some(body.into()));
        let req = Request::new_with_init(&url, &init)?;
        let _ = Fetch::Request(req).send().await;
    }

    Response::ok("ok")
}

fn build_adapter(ctx: &RouteContext<()>) -> Result<TelegramAdapter> {
    Ok(TelegramAdapter::new(
        ctx.env.secret("TG_BOT_TOKEN")?.to_string(),
        ctx.env.var("TG_BOT_USERNAME")?.to_string(),
        ctx.env.secret("TG_WEBHOOK_SECRET")?.to_string(),
    ))
}
