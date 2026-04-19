use worker::*;
use grumps_messaging::adapter::MessagingPlatform;
use grumps_messaging::discord::DiscordAdapter;
use grumps_nlu::parser;
use crate::{db, d1_rest::D1RestClient, provisioning, handler};

pub async fn handle_incoming(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let discord = build_adapter(&ctx)?;
    let body = req.bytes().await?;

    // Discord signature verification (Ed25519 — MVP accepts all, see discord.rs TODO)
    let signature = req.headers().get("X-Signature-Ed25519")?.unwrap_or_default();
    if discord.verify_signature(&body, &signature).is_err() {
        return Response::error("Bad signature", 403);
    }

    let inbound = match discord.parse_webhook(&body) {
        Ok(Some(m)) => m,
        _ => return Response::ok("ok"),
    };

    // KV dedup
    let kv = ctx.kv("KV")?;
    let key = format!("msg:discord:{}", inbound.message_id);
    if kv.get(&key).text().await?.is_some() { return Response::ok("ok"); }
    kv.put(&key, "1")?.expiration_ttl(86400).execute().await?;

    // Resolve/provision workspace
    let index_db = db::get_index_db(&ctx.env)?;
    let d1_client = D1RestClient::from_env(&ctx.env)?;
    let workspace = match db::lookup_workspace(&index_db, "discord", &inbound.channel_id).await? {
        Some(ws) => ws,
        None => {
            let (slug, db_id) = provisioning::provision_workspace(&d1_client, &index_db, "discord", &inbound.channel_id).await?;
            db::WorkspaceMetaRow { slug, d1_database_id: db_id, name: None, plan: "free".into() }
        }
    };

    let ws_db = db::WorkspaceDb::new(&d1_client, workspace.d1_database_id.clone());
    let (member_id, is_first) = ws_db.upsert_member(&inbound.sender_id, &inbound.sender_name).await?;
    let role = if is_first { "admin" } else { "member" };
    let _ = db::upsert_index_user(&index_db, &inbound.sender_id, &workspace.slug, role).await;

    // Discord adapter already normalizes <@APP_ID> to @grumps in parse_webhook
    let text = match &inbound.text { Some(t) => t.as_str(), None => return Response::ok("ok") };

    let is_reply_to_bot = match &inbound.quoted_message_id {
        Some(qid) => ws_db.is_bot_message(qid).await.unwrap_or(false),
        None => false,
    };

    // RAG ingest (best-effort, non-blocking on failure)
    {
        let meta = crate::rag::ChatVectorMetadata {
            workspace_slug: workspace.slug.to_string(),
            platform: "discord".into(),
            sender_member_id: member_id.to_string(),
            sender_name: inbound.sender_name.to_string(),
            text: text.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        if let Err(e) = crate::rag::ingest_message(&ctx.env, &meta).await {
            worker::console_log!("RAG ingest error (discord): {e}");
        }
    }

    let parse_result = parser::parse(text, inbound.is_mention_to_bot, inbound.is_direct_message, is_reply_to_bot, inbound.quoted_message_id.is_some());

    // LLM client (optional)
    let llm = crate::llm_client::LlmClient::from_env(&ctx.env).ok();

    let result = handler::handle_message(
        parse_result, &inbound.message_id,
        inbound.quoted_message_id.as_deref(), inbound.quoted_message_text.as_deref(),
        &inbound.sender_name, &ws_db, &member_id, &workspace.slug,
        llm.as_ref(), &workspace.plan,
    ).await?;

    // Send responses
    for msg in &result.messages {
        let (url, body) = discord.build_send_request(&inbound.channel_id, msg)
            .map_err(|e| Error::RustError(format!("{:?}", e)))?;
        let mut headers = Headers::new();
        headers.set("Content-Type", "application/json")?;
        headers.set("Authorization", &format!("Bot {}", discord.bot_token))?;
        let mut init = RequestInit::new();
        init.with_method(Method::Post).with_headers(headers).with_body(Some(body.into()));
        let req = Request::new_with_init(&url, &init)?;
        let mut resp = Fetch::Request(req).send().await?;
        // Track bot message
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if let Some(msg_id) = json.get("id").and_then(|v| v.as_str()) {
                let _ = ws_db.track_bot_message(msg_id, None).await;
            }
        }
    }

    Response::ok("ok")
}

fn build_adapter(ctx: &RouteContext<()>) -> Result<DiscordAdapter> {
    Ok(DiscordAdapter::new(
        ctx.env.secret("DISCORD_BOT_TOKEN")?.to_string(),
        ctx.env.var("DISCORD_APPLICATION_ID")?.to_string(),
        ctx.env.secret("DISCORD_PUBLIC_KEY")?.to_string(),
    ))
}
