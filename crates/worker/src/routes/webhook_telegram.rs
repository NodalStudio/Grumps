use grumps_i18n::{t, Locale};
use grumps_messaging::adapter::MessagingPlatform;
use grumps_messaging::adapter::MessagingPlatform;
use grumps_messaging::telegram::{TelegramAdapter, TgChatMemberUpdated, TgUpdate};
use grumps_messaging::telegram::{TelegramAdapter, TgChatMemberUpdated, TgUpdate};
use grumps_nlu::parser;
use grumps_nlu::parser;
use worker::*;
use worker::*;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Transition {
    FirstAddAsMember,
    FirstAddAsAdmin,
    Promotion,
    Ignore,
}

/// Classify a bot `my_chat_member` status transition. Pure: no I/O, easy to test.
/// Unknown `old` statuses (empty, "left", "kicked", anything not admin) are
/// treated as "bot wasn't in the group before", so a `new == member` or
/// `new == administrator` counts as a first-add.
pub(crate) fn route_chat_member(old: &str, new: &str) -> Transition {
    let was_in_group_as_non_admin = matches!(old, "member" | "restricted");
    let was_admin = old == "administrator";
    match (was_admin, was_in_group_as_non_admin, new) {
        (_, true, "administrator") => Transition::Promotion,
        (false, false, "administrator") => Transition::FirstAddAsAdmin,
        (false, false, "member") => Transition::FirstAddAsMember,
        _ => Transition::Ignore,
    }
}

pub async fn handle_incoming(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let tg = build_adapter(&ctx)?;
    let body = req.bytes().await?;

    // Verify secret token. Header is mandatory: an attacker who omits it
    // would otherwise skip verification entirely and inject arbitrary updates.
    let secret = req
        .headers()
        .get("X-Telegram-Bot-Api-Secret-Token")?
        .ok_or_else(|| Error::RustError("missing X-Telegram-Bot-Api-Secret-Token header".into()))?;
    if tg.verify_signature(&body, &secret).is_err() {
        return Response::error("Bad secret", 403);
    }

    // Typed parse of my_chat_member — routes based on status transition.
    if let Ok(update) = serde_json::from_slice::<TgUpdate>(&body) {
        if let Some(mcm) = update.my_chat_member {
            let transition =
                route_chat_member(&mcm.old_chat_member.status, &mcm.new_chat_member.status);
            let new_status = mcm.new_chat_member.status.as_str();
            if matches!(new_status, "left" | "kicked" | "banned") {
                let chat_id = mcm.chat.id.to_string();
                let index_db = db::get_index_db(&ctx.env)?;
                if let Some(ws) = db::lookup_workspace(&index_db, "telegram", &chat_id).await? {
                    let _ = db::archive_workspace(&index_db, &ws.slug).await;
                    crate::observability::log_event(
                        "workspace.archived",
                        &serde_json::json!({ "slug": ws.slug, "reason": new_status }),
                    );
                }
                return Response::ok("ok");
            }
            return match transition {
                Transition::FirstAddAsMember => handle_first_add(&ctx, &tg, &mcm, false).await,
                Transition::FirstAddAsAdmin => handle_first_add(&ctx, &tg, &mcm, true).await,
                Transition::Promotion => handle_promotion(&ctx, &tg, &mcm).await,
                Transition::Ignore => Response::ok("ok"),
            };
        }
    }

    // DM auto-provisioning: a private-chat message from a user we haven't seen
    // in that DM before creates a personal workspace with the user as sole admin.
    if let Ok(update) = serde_json::from_slice::<TgUpdate>(&body) {
        if let Some(msg) = &update.message {
            if msg.chat.chat_type == "private" {
                if let Some(from) = &msg.from {
                    let chat_id = msg.chat.id.to_string();
                    let index_db = db::get_index_db(&ctx.env)?;

                    if db::lookup_workspace(&index_db, "telegram", &chat_id)
                        .await?
                        .is_none()
                    {
                        // First contact — provision the DM workspace and greet.
                        let d1_client = D1RestClient::from_env(&ctx.env)?;
                        let locale =
                            Locale::from_code(from.language_code.as_deref().unwrap_or("en"));
                        let default_name = t(locale, "workspace.default_name.personal", &[]);

                        let (slug, _db_id) = provisioning::provision_workspace_dm(
                            &d1_client,
                            &index_db,
                            "telegram",
                            &chat_id,
                            Some(&default_name),
                        )
                        .await?;

                        let _ = db::update_workspace_locale(&index_db, &slug, locale.code()).await;

                        let tg_user_id = from.id.to_string();
                        let display_name = format_tg_display_name(from);
                        let _ = db::upsert_identity_user(
                            &index_db,
                            "telegram",
                            &tg_user_id,
                            &slug,
                            "admin",
                            Some(&display_name),
                        )
                        .await;

                        crate::observability::log_event(
                            "workspace.dm_provisioned",
                            &serde_json::json!({ "slug": slug, "tg_user_id": tg_user_id }),
                        );

                        let welcome = t(
                            locale,
                            "telegram.onboarding.dm_welcome",
                            &[("slug", &slug), ("bot", &tg.bot_username)],
                        );
                        let _ = send_message(
                            &tg,
                            &chat_id,
                            &grumps_messaging::adapter::OutboundMessage {
                                text: welcome,
                                reply_to: None,
                            },
                        )
                        .await;

                        return Response::ok("ok");
                    }
                    // DM workspace already exists — fall through so the normal flow
                    // resolves it and runs the agent routing.
                }
            }
        }
    }

    let inbound = match tg.parse_webhook(&body) {
        Ok(Some(m)) => m,
        _ => return Response::ok("ok"),
    };

    // KV dedup
    let kv = ctx.kv("KV")?;
    let key = format!("msg:tg:{}", inbound.message_id);
    if kv.get(&key).text().await?.is_some() {
        return Response::ok("ok");
    }
    kv.put(&key, "1")?.expiration_ttl(86400).execute().await?;

    // Resolve/provision workspace
    let index_db = db::get_index_db(&ctx.env)?;
    let d1_client = D1RestClient::from_env(&ctx.env)?;
    let workspace = match db::lookup_workspace(&index_db, "telegram", &inbound.channel_id).await? {
        Some(ws) => ws,
        None => {
            let (slug, db_id) = provisioning::provision_workspace(
                &d1_client,
                &index_db,
                "telegram",
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
    let (member_id, is_first) = ws_db
        .upsert_member(&inbound.sender_id, &inbound.sender_name)
        .await?;
    let role = if is_first { "admin" } else { "member" };
    let _ = db::upsert_identity_user(
        &index_db,
        "telegram",
        &inbound.sender_id,
        &workspace.slug,
        role,
        Some(&inbound.sender_name),
    )
    .await;

    let text = match &inbound.text {
        Some(t) => t.as_str(),
        None => return Response::ok("ok"),
    };

    // Strip bot mention and normalize to @grumps so existing NLU parser works unchanged
    let clean_text = text
        .replace(&format!("@{}", tg.bot_username), "@grumps")
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
        &clean_text,
        inbound.is_mention_to_bot,
        inbound.is_direct_message,
        is_reply_to_bot,
        inbound.quoted_message_id.is_some(),
    );

    // LLM client (optional)
    let llm = crate::llm_client::LlmClient::from_env(&ctx.env).ok();

    let result = handler::handle_message(
        Some(&ctx.env),
        &clean_text,
        parse_result,
        &inbound.message_id,
        inbound.quoted_message_id.as_deref(),
        inbound.quoted_message_text.as_deref(),
        &inbound.sender_name,
        &ws_db,
        &member_id,
        &workspace.slug,
        &workspace.locale,
        llm.as_ref(),
        &workspace.plan,
    )
    .await?;

    // Send responses
    for msg in &result.messages {
        let (url, body) = tg
            .build_send_request(&inbound.channel_id, msg)
            .map_err(|e| Error::RustError(format!("{:?}", e)))?;
        let headers = Headers::new();
        headers.set("Content-Type", "application/json")?;
        let mut init = RequestInit::new();
        init.with_method(Method::Post)
            .with_headers(headers)
            .with_body(Some(body.into()));
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

async fn handle_first_add(
    ctx: &RouteContext<()>,
    tg: &TelegramAdapter,
    mcm: &TgChatMemberUpdated,
    added_as_admin: bool,
) -> Result<Response> {
    let index_db = db::get_index_db(&ctx.env)?;
    let d1_client = D1RestClient::from_env(&ctx.env)?;

    let chat_id = mcm.chat.id.to_string();
    let locale = Locale::from_code(mcm.from.language_code.as_deref().unwrap_or("en"));

    // Provision workspace (idempotent: lookup first, then insert).
    let slug = match db::lookup_workspace(&index_db, "telegram", &chat_id).await? {
        Some(ws) => ws.slug,
        None => {
            let chat_title = mcm.chat.title.as_deref();
            let (slug, _db_id) = provisioning::provision_workspace_with_meta(
                &d1_client, &index_db, "telegram", &chat_id, chat_title, false,
            )
            .await?;
            slug
        }
    };

    // Persist the resolved locale on the workspace row.
    let _ = db::update_workspace_locale(&index_db, &slug, locale.code()).await;

    // If added as admin, set description in the resolved locale.
    if added_as_admin {
        let description = t(
            locale,
            "telegram.onboarding.description",
            &[("slug", &slug)],
        );
        let _ = call_set_description(tg, &chat_id, &description).await;
    }

    // Pick welcome variant based on admin status.
    let welcome_key = if added_as_admin {
        "telegram.onboarding.welcome.added_as_admin"
    } else {
        "telegram.onboarding.welcome.added_as_member"
    };
    let welcome = t(
        locale,
        welcome_key,
        &[("slug", &slug), ("bot", &tg.bot_username)],
    );

    let msg = grumps_messaging::adapter::OutboundMessage {
        text: welcome,
        reply_to: None,
    };
    let _ = send_message(tg, &chat_id, &msg).await;

    // Link the TG user who added the bot to the workspace + register their identity.
    let tg_user_id = mcm.from.id.to_string();
    let display_name = format_tg_display_name(&mcm.from);
    let role = if added_as_admin { "admin" } else { "member" };
    let _ = crate::db::upsert_identity_user(
        &index_db,
        "telegram",
        &tg_user_id,
        &slug,
        role,
        Some(&display_name),
    )
    .await;

    Response::ok("ok")
}

async fn handle_promotion(
    ctx: &RouteContext<()>,
    tg: &TelegramAdapter,
    mcm: &TgChatMemberUpdated,
) -> Result<Response> {
    let index_db = db::get_index_db(&ctx.env)?;
    let chat_id = mcm.chat.id.to_string();

    // Workspace must exist (bot was already in the group before promotion).
    // If not, fall through silently (unusual edge case, shouldn't happen).
    let ws = match db::lookup_workspace(&index_db, "telegram", &chat_id).await? {
        Some(ws) => ws,
        None => return Response::ok("ok"),
    };
    let locale = Locale::from_code(&ws.locale);

    // Re-apply setChatDescription in the workspace locale.
    let description = t(
        locale,
        "telegram.onboarding.description",
        &[("slug", &ws.slug)],
    );
    let description_ok = call_set_description(tg, &chat_id, &description)
        .await
        .unwrap_or(false);

    // V3 picks variant based on whether setChatDescription actually succeeded.
    let key = if description_ok {
        "telegram.onboarding.promoted.with_description"
    } else {
        "telegram.onboarding.promoted.without_description"
    };
    let text = t(locale, key, &[]);

    let msg = grumps_messaging::adapter::OutboundMessage {
        text,
        reply_to: None,
    };
    let _ = send_message(tg, &chat_id, &msg).await;

    // The promoted user becomes workspace admin.
    let tg_user_id = mcm.new_chat_member.user.id.to_string();
    let display_name = format_tg_display_name(&mcm.new_chat_member.user);
    let _ = crate::db::upsert_identity_user(
        &index_db,
        "telegram",
        &tg_user_id,
        &ws.slug,
        "admin",
        Some(&display_name),
    )
    .await;

    Response::ok("ok")
}

fn build_adapter(ctx: &RouteContext<()>) -> Result<TelegramAdapter> {
    Ok(TelegramAdapter::new(
        ctx.env.secret("TG_BOT_TOKEN")?.to_string(),
        ctx.env.var("TG_BOT_USERNAME")?.to_string(),
        ctx.env.secret("TG_WEBHOOK_SECRET")?.to_string(),
    ))
}

/// Call setChatDescription, parse `{ ok: bool }` from the response.
/// Returns `Ok(true)` if Telegram returned `ok: true`, `Ok(false)` on any
/// other response or error. Never fails except on local request-build error.
pub(crate) async fn call_set_description(
    tg: &TelegramAdapter,
    chat_id: &str,
    description: &str,
) -> Result<bool> {
    let (url, body) = tg
        .build_set_description_request(chat_id, description)
        .map_err(|e| Error::RustError(format!("{:?}", e)))?;
    let headers = Headers::new();
    headers.set("Content-Type", "application/json")?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(body.into()));
    let req = Request::new_with_init(&url, &init)?;
    let mut resp = match Fetch::Request(req).send().await {
        Ok(r) => r,
        Err(e) => {
            worker::console_log!("setChatDescription network error: {}", e);
            return Ok(false);
        }
    };
    match resp.json::<serde_json::Value>().await {
        Ok(v) => {
            let ok = v.get("ok").and_then(|b| b.as_bool()).unwrap_or(false);
            if !ok {
                worker::console_log!("setChatDescription failed: {}", v);
            }
            Ok(ok)
        }
        Err(e) => {
            worker::console_log!("setChatDescription: non-JSON response: {}", e);
            Ok(false)
        }
    }
}

/// Send a text message to Telegram. Logs on failure but never propagates
/// — the webhook must return 200 OK to prevent Telegram retry storms.
async fn send_message(
    tg: &TelegramAdapter,
    chat_id: &str,
    msg: &grumps_messaging::adapter::OutboundMessage,
) -> Result<()> {
    let (url, body) = tg
        .build_send_request(chat_id, msg)
        .map_err(|e| Error::RustError(format!("{:?}", e)))?;
    let headers = Headers::new();
    headers.set("Content-Type", "application/json")?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(body.into()));
    let req = Request::new_with_init(&url, &init)?;
    if let Err(e) = Fetch::Request(req).send().await {
        worker::console_log!("sendMessage failed: {}", e);
    }
    Ok(())
}

fn format_tg_display_name(u: &grumps_messaging::telegram::TgUser) -> String {
    let joined = format!("{} {}", u.first_name, u.last_name.as_deref().unwrap_or(""));
    let trimmed = joined.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    if let Some(n) = &u.username {
        if !n.is_empty() {
            return n.clone();
        }
    }
    format!("telegram:{}", u.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_first_add_as_member() {
        assert_eq!(
            route_chat_member("left", "member"),
            Transition::FirstAddAsMember
        );
        assert_eq!(
            route_chat_member("kicked", "member"),
            Transition::FirstAddAsMember
        );
    }

    #[test]
    fn routes_first_add_as_admin() {
        assert_eq!(
            route_chat_member("left", "administrator"),
            Transition::FirstAddAsAdmin
        );
        assert_eq!(
            route_chat_member("kicked", "administrator"),
            Transition::FirstAddAsAdmin
        );
    }

    #[test]
    fn routes_promotion() {
        assert_eq!(
            route_chat_member("member", "administrator"),
            Transition::Promotion
        );
        assert_eq!(
            route_chat_member("restricted", "administrator"),
            Transition::Promotion
        );
    }

    #[test]
    fn ignores_demotion() {
        assert_eq!(
            route_chat_member("administrator", "member"),
            Transition::Ignore
        );
        assert_eq!(
            route_chat_member("administrator", "left"),
            Transition::Ignore
        );
        assert_eq!(
            route_chat_member("administrator", "kicked"),
            Transition::Ignore
        );
    }

    #[test]
    fn ignores_noop_and_unknown() {
        assert_eq!(route_chat_member("member", "member"), Transition::Ignore);
        assert_eq!(
            route_chat_member("administrator", "administrator"),
            Transition::Ignore
        );
        assert_eq!(
            route_chat_member("garbage", "member"),
            Transition::FirstAddAsMember
        );
        assert_eq!(
            route_chat_member("", "administrator"),
            Transition::FirstAddAsAdmin
        );
    }
}
