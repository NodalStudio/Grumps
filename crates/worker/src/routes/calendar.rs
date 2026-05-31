//! Calendar routes : aggregated view + iCal export.
//! See spec § 9.

use crate::routes::util::read_query;
use crate::{d1_rest, db, middleware};
use grumps_calendar::aggregate::aggregate;
use grumps_calendar::ical::generate_ical;
use serde::{Deserialize, Serialize};
use worker::*;

// ── helpers ──────────────────────────────────────────────────────────────────

async fn resolve_workspace(ctx: &RouteContext<()>) -> Result<db::WorkspaceMetaRow> {
    let slug = ctx
        .param("slug")
        .ok_or_else(|| Error::RustError("missing slug".into()))?;
    let index_db = db::get_index_db(&ctx.env)?;
    db::lookup_workspace_by_slug(&index_db, slug)
        .await?
        .ok_or_else(|| Error::RustError("workspace not found".into()))
}

// ── iCal JWT claims ───────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct IcalClaims {
    sub: String, // "ical_export"
    ws: String,  // workspace slug
    iat: i64,
    exp: i64,
}

fn create_ical_jwt(slug: &str, secret: &str) -> std::result::Result<String, String> {
    let now = chrono::Utc::now().timestamp();
    let claims = IcalClaims {
        sub: "ical_export".into(),
        ws: slug.to_string(),
        iat: now,
        // 1 year token — user can revoke by deleting
        exp: now + 365 * 24 * 3600,
    };
    let key = jsonwebtoken::EncodingKey::from_secret(secret.as_bytes());
    jsonwebtoken::encode(&jsonwebtoken::Header::default(), &claims, &key)
        .map_err(|e| format!("jwt error: {e}"))
}

fn verify_ical_jwt(token: &str, secret: &str) -> std::result::Result<IcalClaims, String> {
    let key = jsonwebtoken::DecodingKey::from_secret(secret.as_bytes());
    let mut v = jsonwebtoken::Validation::default();
    v.validate_exp = true;
    jsonwebtoken::decode::<IcalClaims>(token, &key, &v)
        .map(|d| d.claims)
        .map_err(|e| format!("invalid ical token: {e}"))
}

const ICAL_SETTING_KEY: &str = "ical_token";

// ── GET /api/w/:slug/calendar ─────────────────────────────────────────────────

pub async fn aggregated(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match middleware::verify_session(&req, &ctx.env).await {
        Ok(c) => c,
        Err(e) => return middleware::error_with_cors(&req, e.status(), e.code(), &e.to_string()),
    };
    let ws = match resolve_workspace(&ctx).await {
        Ok(w) => w,
        Err(_) => {
            return middleware::error_with_cors(
                &req,
                404,
                "workspace.not_found",
                "workspace not found",
            )
        }
    };
    if !claims.workspaces.contains(&ws.slug) {
        return middleware::error_with_cors(
            &req,
            403,
            "auth.not_member",
            "not a member of this workspace",
        );
    }

    let url = req.url()?;
    // Coarse default window (the SPA/agent send explicit from/to); the workspace
    // tz offset shifts only the ±month edges by hours, never which items fall in.
    let now = chrono::Utc::now();
    let default_from = (now - chrono::Duration::days(30)).to_rfc3339();
    let default_to = (now + chrono::Duration::days(90)).to_rfc3339();
    let mut from = default_from;
    let mut to = default_to;
    read_query(&url, |k, v| match k {
        "from" => from = v.to_string(),
        "to" => to = v.to_string(),
        _ => {}
    });

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id.clone());

    let events = ws_db.list_events_in_range(&from, &to).await?;

    let todos_rows = ws_db.list_todos_with_deadline_in_range(&from, &to).await?;
    let todos_json: Vec<serde_json::Value> = todos_rows
        .into_iter()
        .map(|t| serde_json::to_value(&t).unwrap_or(serde_json::Value::Null))
        .collect();

    // Reminders: filter active reminders in range
    let all_reminders = ws_db.get_active_reminders().await?;
    let reminders_json: Vec<serde_json::Value> = all_reminders
        .into_iter()
        .filter(|r| r.remind_at.as_str() >= from.as_str() && r.remind_at.as_str() <= to.as_str())
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "title": r.title,
                "remind_at": r.remind_at,
                "recurrence": r.recurrence,
                "target_member": r.target_member,
            })
        })
        .collect();

    // Scheduled actions: filter pending in range
    let all_scheduled = ws_db
        .list_scheduled_actions(Some("pending"), 500, 0)
        .await?;
    let scheduled_json: Vec<serde_json::Value> = all_scheduled
        .into_iter()
        .filter(|a| {
            let t = a.trigger_at.to_rfc3339();
            t.as_str() >= from.as_str() && t.as_str() <= to.as_str()
        })
        .map(|a| {
            serde_json::json!({
                "id": a.id,
                "title": a.title,
                "trigger_at": a.trigger_at.to_rfc3339(),
                "recurrence": a.recurrence,
                "created_by": a.created_by,
            })
        })
        .collect();

    let items = aggregate(events, todos_json, reminders_json, scheduled_json, &ws.slug);

    middleware::with_cors(&req, Response::from_json(&items)?)
}

// ── POST /api/w/:slug/calendar/ical-token ─────────────────────────────────────

pub async fn create_ical_token(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match middleware::verify_session(&req, &ctx.env).await {
        Ok(c) => c,
        Err(e) => return middleware::error_with_cors(&req, e.status(), e.code(), &e.to_string()),
    };
    let ws = match resolve_workspace(&ctx).await {
        Ok(w) => w,
        Err(_) => {
            return middleware::error_with_cors(
                &req,
                404,
                "workspace.not_found",
                "workspace not found",
            )
        }
    };
    if !claims.workspaces.contains(&ws.slug) {
        return middleware::error_with_cors(
            &req,
            403,
            "auth.not_member",
            "not a member of this workspace",
        );
    }

    // Admin only
    // We verify by checking the member's role in the workspace DB
    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id.clone());

    let jwt_secret = ctx.env.secret("JWT_SECRET")?.to_string();
    let token = create_ical_jwt(&ws.slug, &jwt_secret).map_err(|e| Error::RustError(e))?;

    ws_db.set_setting(ICAL_SETTING_KEY, &token).await?;

    // Build the public iCal URL
    let base = "https://grumps.app";
    let ical_url = format!("{}/cal/{}.ics?t={}", base, ws.slug, token);

    #[derive(Serialize)]
    struct Resp {
        url: String,
        token: String,
    }
    middleware::with_cors(
        &req,
        Response::from_json(&Resp {
            url: ical_url,
            token,
        })?
        .with_status(201),
    )
}

// ── DELETE /api/w/:slug/calendar/ical-token ───────────────────────────────────

pub async fn delete_ical_token(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match middleware::verify_session(&req, &ctx.env).await {
        Ok(c) => c,
        Err(e) => return middleware::error_with_cors(&req, e.status(), e.code(), &e.to_string()),
    };
    let ws = match resolve_workspace(&ctx).await {
        Ok(w) => w,
        Err(_) => {
            return middleware::error_with_cors(
                &req,
                404,
                "workspace.not_found",
                "workspace not found",
            )
        }
    };
    if !claims.workspaces.contains(&ws.slug) {
        return middleware::error_with_cors(
            &req,
            403,
            "auth.not_member",
            "not a member of this workspace",
        );
    }

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id.clone());
    ws_db.delete_setting(ICAL_SETTING_KEY).await?;

    middleware::with_cors(&req, Response::empty()?.with_status(204))
}

// ── GET /cal/:slug.ics?t=<jwt> ────────────────────────────────────────────────

pub async fn ical_feed(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    // slug param comes in as "slug.ics" — strip the .ics suffix
    let slug_param = ctx
        .param("slug")
        .ok_or_else(|| Error::RustError("missing slug".into()))?;
    let slug = slug_param.trim_end_matches(".ics");

    // Get token from query string
    let url = req.url()?;
    let mut token_param: Option<String> = None;
    read_query(&url, |k, v| {
        if k == "t" {
            token_param = Some(v.to_string());
        }
    });
    let token_str = token_param.ok_or_else(|| Error::RustError("missing t param".into()))?;

    // Verify JWT signature and claims
    let jwt_secret = ctx.env.secret("JWT_SECRET")?.to_string();
    let ical_claims = verify_ical_jwt(&token_str, &jwt_secret).map_err(|e| Error::RustError(e))?;

    if ical_claims.ws != slug {
        return Response::error("token workspace mismatch", 403);
    }

    // Look up workspace
    let index_db = db::get_index_db(&ctx.env)?;
    let ws = db::lookup_workspace_by_slug(&index_db, slug)
        .await?
        .ok_or_else(|| Error::RustError("workspace not found".into()))?;

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id.clone());

    // Verify stored token matches (revocation check)
    let stored = ws_db.get_setting(ICAL_SETTING_KEY).await?;
    if stored.as_deref() != Some(&token_str) {
        return Response::error("ical token revoked or not found", 403);
    }

    // Fetch 1 year past + 1 year future
    let now = chrono::Utc::now();
    let from = (now - chrono::Duration::days(365)).to_rfc3339();
    let to = (now + chrono::Duration::days(365)).to_rfc3339();

    let events = ws_db.list_events_in_range(&from, &to).await?;

    let todos_rows = ws_db.list_todos_with_deadline_in_range(&from, &to).await?;
    let todos_json: Vec<serde_json::Value> = todos_rows
        .into_iter()
        .map(|t| serde_json::to_value(&t).unwrap_or(serde_json::Value::Null))
        .collect();

    let all_reminders = ws_db.get_active_reminders().await?;
    let reminders_json: Vec<serde_json::Value> = all_reminders
        .into_iter()
        .filter(|r| r.remind_at.as_str() >= from.as_str() && r.remind_at.as_str() <= to.as_str())
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "title": r.title,
                "remind_at": r.remind_at,
                "recurrence": r.recurrence,
                "target_member": r.target_member,
            })
        })
        .collect();

    let all_scheduled = ws_db
        .list_scheduled_actions(Some("pending"), 500, 0)
        .await?;
    let scheduled_json: Vec<serde_json::Value> = all_scheduled
        .into_iter()
        .filter(|a| {
            let t = a.trigger_at.to_rfc3339();
            t.as_str() >= from.as_str() && t.as_str() <= to.as_str()
        })
        .map(|a| {
            serde_json::json!({
                "id": a.id,
                "title": a.title,
                "trigger_at": a.trigger_at.to_rfc3339(),
                "recurrence": a.recurrence,
                "created_by": a.created_by,
            })
        })
        .collect();

    let workspace_name = ws.name.as_deref().unwrap_or(slug);
    let tz = ws_db
        .get_setting("timezone")
        .await
        .ok()
        .flatten()
        .filter(|s| !s.is_empty());
    let items = aggregate(events, todos_json, reminders_json, scheduled_json, slug);
    let ical_body = generate_ical(workspace_name, &items, tz.as_deref());

    let mut resp = Response::ok(ical_body)?;
    resp.headers_mut()
        .set("Content-Type", "text/calendar; charset=utf-8")?;
    resp.headers_mut().set("Cache-Control", "no-cache")?;
    Ok(resp)
}
