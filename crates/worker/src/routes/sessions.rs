use crate::middleware::error_with_cors;
use crate::{db, middleware, observability::log_event};
use serde::Serialize;
use worker::*;

#[derive(Serialize)]
struct SessionDto {
    id: String,
    device_label: Option<String>,
    country_hint: Option<String>,
    created_at: String,
    last_seen_at: String,
    is_current: bool,
}

pub async fn list_sessions(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match middleware::verify_session(&req, &ctx.env).await {
        Ok(c) => c,
        Err(e) => return error_with_cors(&req, e.status(), e.code(), &e.to_string()),
    };

    let index_db = db::get_index_db(&ctx.env)?;
    let rows = db::list_active_sessions(&index_db, &claims.sub).await?;
    let current_sid = claims.sid.unwrap_or_default();

    let sessions: Vec<SessionDto> = rows
        .into_iter()
        .map(|r| SessionDto {
            id: r.id.clone(),
            device_label: Some(r.device_label.unwrap_or_default()).filter(|s| !s.is_empty()),
            country_hint: Some(r.country_hint.unwrap_or_default()).filter(|s| !s.is_empty()),
            created_at: r.created_at,
            last_seen_at: r.last_seen_at,
            is_current: r.id == current_sid,
        })
        .collect();

    let mut resp = Response::from_json(&serde_json::json!({ "sessions": sessions }))?;
    let origin = req.headers().get("Origin")?.unwrap_or_default();
    middleware::add_cors(&mut resp, Some(&origin))?;
    Ok(resp)
}

pub async fn revoke_specific(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match middleware::verify_session(&req, &ctx.env).await {
        Ok(c) => c,
        Err(e) => return error_with_cors(&req, e.status(), e.code(), &e.to_string()),
    };
    let sid = match ctx.param("id") {
        Some(s) => s.clone(),
        None => return error_with_cors(&req, 400, "bad_request", "missing session id"),
    };

    let index_db = db::get_index_db(&ctx.env)?;
    let revoked = db::revoke_session(&index_db, &sid, &claims.sub).await?;
    if !revoked {
        return error_with_cors(
            &req,
            404,
            "session.not_found",
            "session not found or already revoked",
        );
    }

    let _ = middleware::invalidate_session_cache(&ctx.env, &sid).await;
    log_event(
        "auth.session_revoked",
        &serde_json::json!({
            "user_id": claims.sub, "sid": sid, "reason": "revoke",
        }),
    );

    let mut resp = Response::from_json(&serde_json::json!({ "ok": true }))?;
    let origin = req.headers().get("Origin")?.unwrap_or_default();
    middleware::add_cors(&mut resp, Some(&origin))?;
    Ok(resp)
}

pub async fn revoke_all_others(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match middleware::verify_session(&req, &ctx.env).await {
        Ok(c) => c,
        Err(e) => return error_with_cors(&req, e.status(), e.code(), &e.to_string()),
    };
    let current_sid = claims.sid.clone().unwrap_or_default();

    let index_db = db::get_index_db(&ctx.env)?;
    let count = db::revoke_other_sessions(&index_db, &claims.sub, &current_sid).await?;

    log_event(
        "auth.session_revoked",
        &serde_json::json!({
            "user_id": claims.sub, "reason": "revoke_all", "count": count,
        }),
    );

    let mut resp = Response::from_json(&serde_json::json!({ "ok": true, "revoked": count }))?;
    let origin = req.headers().get("Origin")?.unwrap_or_default();
    middleware::add_cors(&mut resp, Some(&origin))?;
    Ok(resp)
}
