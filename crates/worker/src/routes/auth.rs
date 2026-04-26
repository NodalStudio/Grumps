use worker::*;
use serde::{Deserialize, Serialize};
use grumps_messaging::adapter::MessagingPlatform;
use crate::{db, middleware::{self, error_with_cors, AuthError}};
use crate::auth_telegram::{TelegramWidgetPayload, verify_widget_hash};
use crate::rate_limit::check_rate_limit;
use crate::observability::log_event;

#[allow(dead_code)]
type _AuthErrorMarker = AuthError;

#[derive(Deserialize)]
struct OtpRequest {
    phone: String,
    workspace_slug: Option<String>,
}

#[derive(Deserialize)]
struct VerifyRequest {
    phone: String,
    code: String,
}

#[derive(Serialize)]
struct OtpResponse {
    ok: bool,
}

#[derive(Serialize)]
struct VerifyResponse {
    token: String,
    user_id: String,
    workspaces: Vec<WorkspaceInfo>,
}

#[derive(Serialize)]
struct WorkspaceInfo {
    slug: String,
    name: Option<String>,
    role: String,
}

/// POST /auth/otp — send OTP code via WhatsApp
pub async fn handle_send_otp(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let origin = req.headers().get("Origin")?.unwrap_or_default();
    let body: OtpRequest = req.json().await.map_err(|_| Error::RustError("invalid json".into()))?;

    let phone = body.phone.trim().to_string();
    if phone.is_empty() {
        let mut resp = Response::error("phone required", 400)?;
        middleware::add_cors(&mut resp, Some(&origin))?;
        return Ok(resp);
    }

    // Generate 6-digit OTP
    let code = generate_otp();

    // Store in KV (TTL 5 minutes)
    let kv = ctx.kv("KV")?;
    let otp_key = format!("otp:{}", phone);
    kv.put(&otp_key, &code)?.expiration_ttl(300).execute().await?;

    // Send via WhatsApp
    let wa = crate::routes::webhook::build_adapter_from_env(&ctx.env)?;
    let msg = grumps_messaging::adapter::OutboundMessage {
        text: format!("Your Grumps verification code: {}\n\nExpires in 5 minutes.", code),
        reply_to: None,
    };
    let (url, body_str) = wa.build_send_request(&phone, &msg)
        .map_err(|e| Error::RustError(format!("{:?}", e)))?;

    let headers = Headers::new();
    headers.set("Authorization", &format!("Bearer {}", wa.access_token))?;
    headers.set("Content-Type", "application/json")?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post).with_headers(headers).with_body(Some(worker::wasm_bindgen::JsValue::from_str(&body_str)));
    let send_req = Request::new_with_init(&url, &init)?;
    let _ = Fetch::Request(send_req).send().await?;

    let mut resp = Response::from_json(&OtpResponse { ok: true })?;
    middleware::add_cors(&mut resp, Some(&origin))?;
    Ok(resp)
}

/// POST /auth/verify — verify OTP, return JWT
pub async fn handle_verify_otp(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let origin = req.headers().get("Origin")?.unwrap_or_default();
    let body: VerifyRequest = req.json().await.map_err(|_| Error::RustError("invalid json".into()))?;

    let phone = body.phone.trim().to_string();
    let code = body.code.trim().to_string();

    // Check OTP from KV
    let kv = ctx.kv("KV")?;
    let otp_key = format!("otp:{}", phone);
    let stored = kv.get(&otp_key).text().await?;

    match stored {
        Some(stored_code) if stored_code == code => {
            // Delete used OTP
            kv.delete(&otp_key).await?;

            // Lookup user and workspaces from Index DB
            let index_db = db::get_index_db(&ctx.env)?;

            #[derive(serde::Deserialize)]
            struct UserRow { id: String }
            let user = index_db.prepare("SELECT id FROM users WHERE phone = ?1")
                .bind(&[phone.clone().into()])?.first::<UserRow>(None).await?;

            let user_id = match user {
                Some(u) => u.id,
                None => {
                    let mut resp = Response::error("phone not registered", 404)?;
                    middleware::add_cors(&mut resp, Some(&origin))?;
                    return Ok(resp);
                }
            };

            // Get user's workspaces
            #[derive(serde::Deserialize)]
            struct WsRow { workspace_slug: String, role: String }
            let ws_results = index_db.prepare(
                "SELECT workspace_slug, role FROM user_workspaces WHERE user_id = ?1"
            ).bind(&[user_id.clone().into()])?.all().await?;
            let ws_rows: Vec<WsRow> = ws_results.results()?;

            let workspace_slugs: Vec<String> = ws_rows.iter().map(|w| w.workspace_slug.clone()).collect();

            // Get workspace names
            let mut workspaces = Vec::new();
            for ws in &ws_rows {
                #[derive(serde::Deserialize)]
                struct NameRow { name: Option<String> }
                let name_row = index_db.prepare("SELECT name FROM workspaces_meta WHERE slug = ?1")
                    .bind(&[ws.workspace_slug.clone().into()])?.first::<NameRow>(None).await?;
                workspaces.push(WorkspaceInfo {
                    slug: ws.workspace_slug.clone(),
                    name: name_row.and_then(|r| r.name),
                    role: ws.role.clone(),
                });
            }

            // Create JWT
            let jwt_secret = ctx.env.secret("JWT_SECRET")?.to_string();
            let token = middleware::create_jwt(&user_id, &phone, workspace_slugs, &jwt_secret)
                .map_err(|e| Error::RustError(e))?;

            let mut resp = Response::from_json(&VerifyResponse { token, user_id, workspaces })?;
            middleware::add_cors(&mut resp, Some(&origin))?;
            Ok(resp)
        }
        _ => {
            let mut resp = Response::error("invalid or expired code", 401)?;
            middleware::add_cors(&mut resp, Some(&origin))?;
            Ok(resp)
        }
    }
}

/// Generate a random 6-digit OTP.
fn generate_otp() -> String {
    // Use uuid as entropy source (no rand crate needed on wasm32)
    let id = uuid::Uuid::new_v4();
    let bytes = id.as_bytes();
    let num = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) % 900000 + 100000;
    format!("{}", num)
}

#[derive(Serialize)]
struct WidgetLoginResponse {
    user_id: String,
    display_name: String,
    workspaces: Vec<crate::db::WorkspaceRef>,
    csrf_token: String,
}

pub async fn handle_telegram_verify(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    // Rate limit first to bound session-table growth under spam.
    if check_rate_limit(&ctx.env, &req, "auth_verify", 10).await.is_err() {
        log_event("auth.rate_limited", &serde_json::json!({
            "ip": req.headers().get("CF-Connecting-IP").ok().flatten(),
            "bucket": "auth_verify",
        }));
        return error_with_cors(&req, 429, "auth.rate_limited", "too many attempts, retry in a minute");
    }

    let payload: TelegramWidgetPayload = match req.json().await {
        Ok(p) => p,
        Err(e) => return error_with_cors(&req, 400, "auth.invalid_payload", &format!("{:?}", e)),
    };

    // Dev bypass: ENVIRONMENT != production AND secret set AND dev_bypass flag present.
    let env_kind = ctx.env.var("ENVIRONMENT").map(|v| v.to_string()).unwrap_or_default();
    let dev_ok = env_kind != "production"
        && payload.dev_bypass == Some(true)
        && ctx.env.secret("GRUMPS_DEV_AUTH_BYPASS").is_ok();

    if !dev_ok {
        // Normal HMAC path.
        let bot_token = ctx.env.secret("TG_BOT_TOKEN")
            .map(|v| v.to_string())
            .map_err(|_| Error::RustError("TG_BOT_TOKEN missing".into()))?;
        if !verify_widget_hash(&payload, &bot_token) {
            log_event("auth.hmac_failed", &serde_json::json!({
                "ip": req.headers().get("CF-Connecting-IP").ok().flatten(),
                "tg_id_claimed": payload.id,
            }));
            return error_with_cors(&req, 401, "auth.invalid_hash", "invalid telegram payload");
        }
        let now = chrono::Utc::now().timestamp();
        if now - payload.auth_date > 3600 {
            return error_with_cors(&req, 401, "auth.expired", "login expired, try again");
        }
    }

    let index_db = crate::db::get_index_db(&ctx.env)?;
    let tg_id = payload.id.to_string();
    let display_name = payload.display_name();

    let user_id = match crate::db::lookup_user_by_identity(&index_db, "telegram", &tg_id).await? {
        Some(uid) => uid,
        None => {
            let new_uid = uuid::Uuid::new_v4().to_string();
            index_db
                .prepare("INSERT INTO users (id, display_name) VALUES (?1, ?2)")
                .bind(&[new_uid.clone().into(), display_name.clone().into()])?
                .run().await?;
            index_db
                .prepare("INSERT INTO user_identities (platform, platform_user_id, user_id) VALUES (?1, ?2, ?3)")
                .bind(&[("telegram").into(), tg_id.clone().into(), new_uid.clone().into()])?
                .run().await?;
            log_event("auth.user_created", &serde_json::json!({
                "user_id": new_uid, "platform": "telegram", "platform_user_id": tg_id,
            }));
            new_uid
        }
    };

    // Session row + cookies.
    let sid = uuid::Uuid::new_v4().to_string();
    let ua = req.headers().get("User-Agent").ok().flatten();
    let country = req.headers().get("CF-IPCountry").ok().flatten();
    let device_label = ua.as_deref().map(describe_ua);
    let _ = crate::db::create_session(
        &index_db, &sid, &user_id,
        ua.as_deref(), device_label.as_deref(), country.as_deref(),
    ).await;
    log_event("auth.session_created", &serde_json::json!({
        "user_id": user_id, "sid": sid, "device_label": device_label, "country_hint": country,
    }));

    let workspaces = crate::db::list_user_workspaces_with_names(&index_db, &user_id).await?;
    let slugs: Vec<String> = workspaces.iter().map(|w| w.slug.clone()).collect();

    let jwt_secret = ctx.env.secret("JWT_SECRET")?.to_string();
    let csrf = random_b64_token(32);
    let jwt = middleware::create_jwt_with_csrf(
        &user_id, slugs, &sid, &csrf, Some(tg_id.as_str()), &jwt_secret,
    ).map_err(|e| Error::RustError(e))?;

    let body = WidgetLoginResponse {
        user_id: user_id.clone(),
        display_name: display_name.clone(),
        workspaces,
        csrf_token: csrf.clone(),
    };
    let mut resp = Response::from_json(&body)?;
    middleware::set_auth_cookies(&mut resp, &jwt, &csrf, &env_kind)?;
    let origin = req.headers().get("Origin")?.unwrap_or_default();
    middleware::add_cors(&mut resp, Some(&origin))?;
    Ok(resp)
}

fn describe_ua(ua: &str) -> String {
    let browser = ["Edg/", "Chrome/", "Firefox/", "Safari/"]
        .iter().find(|k| ua.contains(**k))
        .map(|k| k.trim_end_matches('/')).unwrap_or("Browser");
    let os = [("Mac OS X", "macOS"), ("iPhone", "iPhone"), ("Android", "Android"),
              ("Windows", "Windows"), ("Linux", "Linux")]
        .iter().find(|(k, _)| ua.contains(k)).map(|(_, v)| *v).unwrap_or("device");
    format!("{} on {}", browser, os)
}

fn random_b64_token(len_bytes: usize) -> String {
    // No rand crate on wasm — derive entropy from two UUIDs.
    let mut buf = Vec::with_capacity(len_bytes);
    while buf.len() < len_bytes {
        buf.extend_from_slice(uuid::Uuid::new_v4().as_bytes());
    }
    buf.truncate(len_bytes);
    base64_url_encode(&buf)
}

fn base64_url_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    let mut i = 0;
    while i + 3 <= bytes.len() {
        let (a, b, c) = (bytes[i], bytes[i+1], bytes[i+2]);
        out.push(ALPHABET[(a >> 2) as usize] as char);
        out.push(ALPHABET[(((a & 0b11) << 4) | (b >> 4)) as usize] as char);
        out.push(ALPHABET[(((b & 0b1111) << 2) | (c >> 6)) as usize] as char);
        out.push(ALPHABET[(c & 0b111111) as usize] as char);
        i += 3;
    }
    let rem = bytes.len() - i;
    if rem == 1 {
        let a = bytes[i];
        out.push(ALPHABET[(a >> 2) as usize] as char);
        out.push(ALPHABET[((a & 0b11) << 4) as usize] as char);
    } else if rem == 2 {
        let (a, b) = (bytes[i], bytes[i+1]);
        out.push(ALPHABET[(a >> 2) as usize] as char);
        out.push(ALPHABET[(((a & 0b11) << 4) | (b >> 4)) as usize] as char);
        out.push(ALPHABET[((b & 0b1111) << 2) as usize] as char);
    }
    out
}

#[derive(Serialize)]
struct MeResponse {
    user_id: String,
    display_name: String,
    default_locale: Option<String>,
    workspaces: Vec<crate::db::WorkspaceRef>,
    csrf_token: String,
}

pub async fn handle_me(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match middleware::verify_session(&req, &ctx.env).await {
        Ok(c) => c,
        Err(e) => return error_with_cors(&req, e.status(), e.code(), &e.to_string()),
    };

    let index_db = crate::db::get_index_db(&ctx.env)?;

    #[derive(Deserialize)]
    struct U { display_name: Option<String>, default_locale: Option<String> }
    let u: Option<U> = index_db
        .prepare("SELECT display_name, default_locale FROM users WHERE id = ?1")
        .bind(&[claims.sub.clone().into()])?.first::<U>(None).await?;
    if u.is_none() {
        return error_with_cors(&req, 401, "auth.user_gone", "user no longer exists");
    }
    let u = u.unwrap();

    // Fresh workspace list (in case memberships changed since login).
    let workspaces = crate::db::list_user_workspaces_with_names(&index_db, &claims.sub).await?;

    // Touch last_seen_at on the session (throttled via KV to avoid per-request D1 writes).
    if let Some(sid) = &claims.sid {
        touch_session_if_stale(&ctx.env, &index_db, sid).await;
    }

    let body = MeResponse {
        user_id: claims.sub.clone(),
        display_name: u.display_name.unwrap_or_else(|| "".into()),
        default_locale: u.default_locale,
        workspaces,
        csrf_token: claims.csrf.unwrap_or_default(),
    };
    let mut resp = Response::from_json(&body)?;
    let origin = req.headers().get("Origin")?.unwrap_or_default();
    middleware::add_cors(&mut resp, Some(&origin))?;
    Ok(resp)
}

async fn touch_session_if_stale(env: &worker::Env, index_db: &worker::D1Database, sid: &str) {
    let kv = match env.kv("KV") { Ok(k) => k, Err(_) => return };
    let key = format!("session:{}:last_seen", sid);
    if kv.get(&key).text().await.ok().flatten().is_some() { return; }
    let _ = crate::db::touch_session_last_seen(index_db, sid).await;
    let _ = kv.put(&key, "1").ok().map(|p| p.expiration_ttl(300).execute());
}

pub async fn handle_logout(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    // If already unauthenticated, still return 200 with cookies cleared — idempotent.
    let env_kind = ctx.env.var("ENVIRONMENT").map(|v| v.to_string()).unwrap_or_default();
    let mut resp = Response::from_json(&serde_json::json!({ "ok": true }))?;
    middleware::clear_auth_cookies(&mut resp, &env_kind)?;
    let origin = req.headers().get("Origin")?.unwrap_or_default();
    middleware::add_cors(&mut resp, Some(&origin))?;

    if let Ok(claims) = middleware::verify_session(&req, &ctx.env).await {
        if let Some(sid) = &claims.sid {
            let index_db = crate::db::get_index_db(&ctx.env)?;
            let _ = crate::db::revoke_session(&index_db, sid, &claims.sub).await;
            let _ = middleware::invalidate_session_cache(&ctx.env, sid).await;
            log_event("auth.session_revoked", &serde_json::json!({
                "user_id": claims.sub, "sid": sid, "reason": "logout",
            }));
        }
    }

    Ok(resp)
}
