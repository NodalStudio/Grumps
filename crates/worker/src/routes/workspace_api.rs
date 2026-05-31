// crates/worker/src/routes/workspace_api.rs
use crate::{d1_rest, db, middleware};
use grumps_messaging::telegram::TelegramAdapter;
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

// ── GET /api/workspaces ───────────────────────────────────────────────────────

pub async fn list_my_workspaces(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match middleware::verify_session(&req, &ctx.env).await {
        Ok(c) => c,
        Err(e) => return middleware::error_with_cors(&req, e.status(), e.code(), &e.to_string()),
    };

    let index_db = db::get_index_db(&ctx.env)?;
    let mut workspaces = Vec::new();
    for slug in &claims.workspaces {
        if let Some(ws) = db::lookup_workspace_by_slug(&index_db, slug).await? {
            workspaces.push(serde_json::json!({
                "slug": ws.slug,
                "name": ws.name,
                "plan": ws.plan,
            }));
        }
    }

    middleware::with_cors(&req, Response::from_json(&workspaces)?)
}

// ── GET /api/w/:slug ─────────────────────────────────────────────────────────

pub async fn workspace_info(req: Request, ctx: RouteContext<()>) -> Result<Response> {
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
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);
    // The SPA renders all timestamps in this timezone (never the browser's);
    // it also anchors the "done this week" count to the workspace calendar.
    let timezone = ws_db.get_setting("timezone").await.ok().flatten()
        .filter(|s| !s.is_empty()).unwrap_or_else(|| "UTC".into());
    let (open, done_week, notes, files) = ws_db.get_status_counts(&timezone).await?;
    let timezone_source = ws_db.get_setting("timezone_source").await.ok().flatten().unwrap_or_default();

    middleware::with_cors(&req, Response::from_json(&serde_json::json!({
        "slug": ws.slug,
        "name": ws.name,
        "plan": ws.plan,
        "timezone": timezone,
        "timezone_source": timezone_source,
        "stats": {
            "open_todos": open,
            "done_this_week": done_week,
            "notes": notes,
            "files": files,
        }
    }))?)
}

// ── GET /api/w/:slug/history ──────────────────────────────────────────────────

pub async fn workspace_history(req: Request, ctx: RouteContext<()>) -> Result<Response> {
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
    let params: std::collections::HashMap<String, String> = url
        .query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let limit: i64 = params
        .get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(50)
        .min(200);

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);
    let log = ws_db.get_activity_log(limit).await?;

    middleware::with_cors(&req, Response::from_json(&log)?)
}

// ── GET /api/w/:slug/members ──────────────────────────────────────────────────

pub async fn workspace_members(req: Request, ctx: RouteContext<()>) -> Result<Response> {
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
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);
    let members = ws_db.get_members().await?;

    middleware::with_cors(&req, Response::from_json(&members)?)
}

// ── PATCH /api/w/:slug/settings/locale ────────────────────────────────────────

pub async fn update_locale(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
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

    let index_db = db::get_index_db(&ctx.env)?;

    // Workspace-admin-only.
    let is_admin = middleware::is_workspace_admin_by_slug(&index_db, &claims.sub, &ws.slug)
        .await
        .unwrap_or(false);
    if !is_admin {
        return middleware::with_cors(&req, Response::error("forbidden: admin required", 403)?);
    }

    #[derive(serde::Deserialize)]
    struct Body {
        locale: String,
    }
    let body: Body = req
        .json()
        .await
        .map_err(|e| Error::RustError(format!("bad body: {e}")))?;

    // Validate: the request locale must be one of the 14 supported codes.
    // `Locale::from_code` silently falls back to En for unknown input,
    // so compare its output against the request to detect invalid input.
    let resolved = grumps_i18n::Locale::from_code(&body.locale);
    if resolved.code() != body.locale {
        return middleware::with_cors(&req, Response::error("unsupported locale", 400)?);
    }

    db::update_workspace_locale(&index_db, &ws.slug, resolved.code()).await?;

    // Side effect: re-apply the Telegram group description in the new locale.
    // Other platforms are no-ops for now.
    if let Ok(Some((platform, channel_id))) = db::lookup_platform_channel(&index_db, &ws.slug).await
    {
        if platform == "telegram" {
            let tg = build_tg_adapter(&ctx)?;
            let desc = grumps_i18n::t(
                resolved,
                "telegram.onboarding.description",
                &[("slug", &ws.slug)],
            );
            let _ = crate::routes::webhook_telegram::call_set_description(&tg, &channel_id, &desc)
                .await;
        }
    }

    middleware::with_cors(
        &req,
        Response::from_json(&serde_json::json!({
            "ok": true,
            "locale": resolved.code()
        }))?,
    )
}

// ── PATCH /api/w/:slug/settings/timezone ──────────────────────────────────────
//
// Two sources, tracked by the `timezone_source` setting (default → detected →
// admin) so members in different timezones don't fight over the value:
//   - source="detected" (browser auto-detect): writes ONLY if not yet set
//     explicitly. Any member may trigger it; the first detection wins.
//   - source="admin": admin-only, always wins and locks out auto-detect.
pub async fn update_timezone(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match middleware::verify_session(&req, &ctx.env).await {
        Ok(c) => c,
        Err(e) => return middleware::error_with_cors(&req, e.status(), e.code(), &e.to_string()),
    };
    let ws = match resolve_workspace(&ctx).await {
        Ok(w) => w,
        Err(_) => return middleware::error_with_cors(&req, 404, "workspace.not_found", "workspace not found"),
    };
    if !claims.workspaces.contains(&ws.slug) {
        return middleware::error_with_cors(&req, 403, "auth.not_member", "not a member of this workspace");
    }

    #[derive(serde::Deserialize)]
    struct Body { timezone: String, #[serde(default)] source: Option<String> }
    let body: Body = match req.json().await {
        Ok(b) => b,
        Err(_) => return middleware::error_with_cors(&req, 400, "bad_request", "invalid JSON"),
    };

    // Validate: must be a real IANA timezone name.
    if body.timezone.parse::<chrono_tz::Tz>().is_err() {
        return middleware::error_with_cors(&req, 400, "bad_request", "unsupported timezone");
    }
    let source = body.source.as_deref().unwrap_or("detected");

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);
    let current_source = ws_db.get_setting("timezone_source").await.ok().flatten().unwrap_or_default();

    if source == "admin" {
        let index_db = db::get_index_db(&ctx.env)?;
        if !middleware::is_workspace_admin_by_slug(&index_db, &claims.sub, &ws.slug).await.unwrap_or(false) {
            return middleware::error_with_cors(&req, 403, "auth.not_admin", "admin role required");
        }
        ws_db.set_setting("timezone", &body.timezone).await?;
        ws_db.set_setting("timezone_source", "admin").await?;
    } else if current_source.is_empty() || current_source == "default" {
        // First browser detection — adopt it. Later detections are no-ops.
        ws_db.set_setting("timezone", &body.timezone).await?;
        ws_db.set_setting("timezone_source", "detected").await?;
    }

    let effective = ws_db.get_setting("timezone").await.ok().flatten().unwrap_or_else(|| "UTC".into());
    middleware::with_cors(&req, Response::from_json(&serde_json::json!({
        "ok": true,
        "timezone": effective,
    }))?)
}

fn build_tg_adapter(ctx: &RouteContext<()>) -> Result<TelegramAdapter> {
    Ok(TelegramAdapter::new(
        ctx.env.secret("TG_BOT_TOKEN")?.to_string(),
        ctx.env.var("TG_BOT_USERNAME")?.to_string(),
        ctx.env.secret("TG_WEBHOOK_SECRET")?.to_string(),
    ))
}

#[derive(serde::Deserialize)]
struct UpdateMe {
    display_name: Option<String>,
    default_locale: Option<String>,
}

pub async fn update_me(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match middleware::verify_session(&req, &ctx.env).await {
        Ok(c) => c,
        Err(e) => return middleware::error_with_cors(&req, e.status(), e.code(), &e.to_string()),
    };
    let body: UpdateMe = match req.json().await {
        Ok(b) => b,
        Err(_) => return middleware::error_with_cors(&req, 400, "bad_request", "invalid JSON"),
    };

    // Validate locale if provided.
    if let Some(loc) = &body.default_locale {
        if grumps_i18n::Locale::from_code(loc).code() != loc.as_str() {
            return middleware::error_with_cors(&req, 400, "bad_request", "unknown locale");
        }
    }
    // Validate display_name length when provided. Empty / whitespace-only
    // is rejected; over 80 chars is rejected to match workspace-name rules.
    if let Some(name) = &body.display_name {
        let trimmed = name.trim();
        if trimmed.is_empty() || trimmed.len() > 80 {
            return middleware::error_with_cors(
                &req,
                400,
                "bad_request",
                "display_name must be 1-80 chars",
            );
        }
    }

    let index_db = crate::db::get_index_db(&ctx.env)?;
    crate::db::update_user_profile(
        &index_db,
        &claims.sub,
        body.display_name.as_deref(),
        body.default_locale.as_deref(),
    )
    .await?;

    let mut resp = Response::from_json(&serde_json::json!({ "ok": true }))?;
    let origin = req.headers().get("Origin")?.unwrap_or_default();
    middleware::add_cors(&mut resp, Some(&origin))?;
    Ok(resp)
}

#[derive(serde::Deserialize)]
struct UpdateWorkspaceName {
    name: String,
}

pub async fn update_workspace_name(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match middleware::verify_session(&req, &ctx.env).await {
        Ok(c) => c,
        Err(e) => return middleware::error_with_cors(&req, e.status(), e.code(), &e.to_string()),
    };
    let slug = match ctx.param("slug") {
        Some(s) => s.clone(),
        None => return middleware::error_with_cors(&req, 400, "bad_request", "missing slug"),
    };

    // Admin-only: check via index DB.
    let index_db = crate::db::get_index_db(&ctx.env)?;
    if !middleware::is_workspace_admin_by_slug(&index_db, &claims.sub, &slug).await? {
        return middleware::error_with_cors(&req, 403, "auth.not_admin", "admin role required");
    }

    let body: UpdateWorkspaceName = match req.json().await {
        Ok(b) => b,
        Err(_) => return middleware::error_with_cors(&req, 400, "bad_request", "invalid JSON"),
    };

    let trimmed = body.name.trim();
    if trimmed.is_empty() || trimmed.len() > 80 {
        return middleware::error_with_cors(&req, 400, "bad_request", "name must be 1-80 chars");
    }

    crate::db::update_workspace_name(&index_db, &slug, trimmed).await?;

    let mut resp = Response::from_json(&serde_json::json!({ "ok": true }))?;
    let origin = req.headers().get("Origin")?.unwrap_or_default();
    middleware::add_cors(&mut resp, Some(&origin))?;
    Ok(resp)
}
