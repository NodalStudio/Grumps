use worker::*;
use serde::{Deserialize, Serialize};
use grumps_messaging::adapter::MessagingPlatform;
use crate::{db, middleware};

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

    let mut headers = Headers::new();
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
