// crates/worker/src/routes/workspace_api.rs
use crate::extract::{Admin, ApiError, Member, Session};
use crate::{d1_rest, db, middleware};
use grumps_messaging::telegram::TelegramAdapter;
use serde::Deserialize;
use validator::Validate;
use worker::*;

// ── GET /api/workspaces ───────────────────────────────────────────────────────

pub async fn list_my_workspaces(
    req: Request,
    ctx: RouteContext<()>,
    s: Session,
) -> Result<Response> {
    let claims = s.0;
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

pub async fn workspace_info(req: Request, ctx: RouteContext<()>, m: Member) -> Result<Response> {
    let ws = m.ws;
    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);
    // The SPA renders all timestamps in this timezone (never the browser's);
    // it also anchors the "done this week" count to the workspace calendar.
    let timezone = ws_db
        .get_setting("timezone")
        .await
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "UTC".into());
    let (open, done_week, notes, files) = ws_db.get_status_counts(&timezone).await?;
    let timezone_source = ws_db
        .get_setting("timezone_source")
        .await
        .ok()
        .flatten()
        .unwrap_or_default();

    middleware::with_cors(
        &req,
        Response::from_json(&serde_json::json!({
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
        }))?,
    )
}

// ── GET /api/w/:slug/history ──────────────────────────────────────────────────

pub async fn workspace_history(req: Request, ctx: RouteContext<()>, m: Member) -> Result<Response> {
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
    let ws_db = db::WorkspaceDb::new(&client, m.ws.d1_database_id);
    let log = ws_db.get_activity_log(limit).await?;

    middleware::with_cors(&req, Response::from_json(&log)?)
}

// ── GET /api/w/:slug/members ──────────────────────────────────────────────────

pub async fn workspace_members(req: Request, ctx: RouteContext<()>, m: Member) -> Result<Response> {
    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, m.ws.d1_database_id);
    let members = ws_db.get_members().await?;

    middleware::with_cors(&req, Response::from_json(&members)?)
}

// ── PATCH /api/w/:slug/settings/locale ────────────────────────────────────────

#[derive(Deserialize, Validate)]
pub struct LocaleBody {
    locale: String,
}

pub async fn update_locale(
    req: Request,
    ctx: RouteContext<()>,
    a: Admin,
    body: LocaleBody,
) -> Result<Response> {
    let ws = a.ws;
    let index_db = db::get_index_db(&ctx.env)?;

    // Validate: the request locale must be one of the 14 supported codes.
    // `Locale::from_code` silently falls back to En for unknown input,
    // so compare its output against the request to detect invalid input.
    let resolved = grumps_i18n::Locale::from_code(&body.locale);
    if resolved.code() != body.locale {
        return ApiError::bad_request("locale.unsupported").into_response(&req);
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
#[derive(Deserialize, Validate)]
pub struct TimezoneBody {
    timezone: String,
    #[serde(default)]
    source: Option<String>,
}

pub async fn update_timezone(
    req: Request,
    ctx: RouteContext<()>,
    m: Member,
    body: TimezoneBody,
) -> Result<Response> {
    // Validate: must be a real IANA timezone name.
    if body.timezone.parse::<chrono_tz::Tz>().is_err() {
        return ApiError::bad_request("timezone.unsupported").into_response(&req);
    }
    let source = body.source.as_deref().unwrap_or("detected");

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, m.ws.d1_database_id);
    let current_source = ws_db
        .get_setting("timezone_source")
        .await
        .ok()
        .flatten()
        .unwrap_or_default();

    if source == "admin" {
        // The admin source requires the admin role — checked here since the
        // route itself is open to any member for the auto-detect path.
        let index_db = db::get_index_db(&ctx.env)?;
        if !middleware::is_workspace_admin_by_slug(&index_db, &m.claims.sub, &m.ws.slug)
            .await
            .unwrap_or(false)
        {
            return ApiError::forbidden("auth.not_admin").into_response(&req);
        }
        ws_db.set_setting("timezone", &body.timezone).await?;
        ws_db.set_setting("timezone_source", "admin").await?;
    } else if current_source.is_empty() || current_source == "default" {
        // First browser detection — adopt it. Later detections are no-ops.
        ws_db.set_setting("timezone", &body.timezone).await?;
        ws_db.set_setting("timezone_source", "detected").await?;
    }

    let effective = ws_db
        .get_setting("timezone")
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "UTC".into());
    middleware::with_cors(
        &req,
        Response::from_json(&serde_json::json!({
            "ok": true,
            "timezone": effective,
        }))?,
    )
}

// ── GET /api/w/:slug/settings ─────────────────────────────────────────────────
//
// Everything the SPA settings page reads in one call: the editable agent
// preferences (persona/proactive/auto_memory/quiet_mode/auto_recap), plus
// read-only fields it displays (language, timezone, iCal token). Backed by
// the per-workspace key/value `settings` table, except `language` which
// lives on `workspaces_meta` (see `update_locale` above).

pub async fn get_settings(req: Request, ctx: RouteContext<()>, m: Member) -> Result<Response> {
    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, m.ws.d1_database_id.clone());
    let settings = ws_db.get_all_settings().await?;
    let as_bool = |k: &str| settings.get(k).map(|v| v == "true");

    middleware::with_cors(
        &req,
        Response::from_json(&serde_json::json!({
            "language": m.ws.locale,
            "timezone": settings.get("timezone"),
            "quiet_mode": as_bool("quiet_mode"),
            "auto_recap": as_bool("auto_recap"),
            "persona": settings.get("persona"),
            "proactive_mode": as_bool("proactive_mode"),
            "auto_memory": as_bool("auto_memory"),
            "ical_token": settings.get("ical_token"),
            "agent_calls_used": null,
            "agent_calls_limit": null,
            "web_search_used": null,
            "web_search_limit": null,
            "storage_used_mb": null,
            "storage_limit_mb": null,
        }))?,
    )
}

// ── PUT /api/w/:slug/settings ─────────────────────────────────────────────────

#[derive(Deserialize, Validate)]
pub struct UpdateSettingsBody {
    persona: Option<String>,
    proactive_mode: Option<bool>,
    auto_memory: Option<bool>,
    quiet_mode: Option<bool>,
    auto_recap: Option<bool>,
}

pub async fn update_settings(
    req: Request,
    ctx: RouteContext<()>,
    a: Admin,
    body: UpdateSettingsBody,
) -> Result<Response> {
    // `persona` is a closed domain enum, not a shape constraint — checked
    // here rather than in the DTO (same pattern as `todo.status_invalid`).
    if let Some(p) = &body.persona {
        if !matches!(p.as_str(), "grumps" | "assistant" | "coach") {
            return ApiError::bad_request("settings.persona_invalid").into_response(&req);
        }
    }

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, a.ws.d1_database_id);

    if let Some(p) = &body.persona {
        ws_db.set_setting("persona", p).await?;
    }
    if let Some(v) = body.proactive_mode {
        ws_db
            .set_setting("proactive_mode", if v { "true" } else { "false" })
            .await?;
    }
    if let Some(v) = body.auto_memory {
        ws_db
            .set_setting("auto_memory", if v { "true" } else { "false" })
            .await?;
    }
    if let Some(v) = body.quiet_mode {
        ws_db
            .set_setting("quiet_mode", if v { "true" } else { "false" })
            .await?;
    }
    if let Some(v) = body.auto_recap {
        ws_db
            .set_setting("auto_recap", if v { "true" } else { "false" })
            .await?;
    }

    middleware::with_cors(
        &req,
        Response::from_json(&serde_json::json!({ "ok": true }))?,
    )
}

fn build_tg_adapter(ctx: &RouteContext<()>) -> Result<TelegramAdapter> {
    Ok(TelegramAdapter::new(
        ctx.env.secret("TG_BOT_TOKEN")?.to_string(),
        ctx.env.var("TG_BOT_USERNAME")?.to_string(),
        ctx.env.secret("TG_WEBHOOK_SECRET")?.to_string(),
    ))
}

// ── PATCH /api/me ─────────────────────────────────────────────────────────────

#[derive(Deserialize, Validate)]
pub struct UpdateMe {
    display_name: Option<String>,
    default_locale: Option<String>,
}

pub async fn update_me(
    req: Request,
    ctx: RouteContext<()>,
    s: Session,
    body: UpdateMe,
) -> Result<Response> {
    let claims = s.0;

    // Validate locale if provided.
    if let Some(loc) = &body.default_locale {
        if grumps_i18n::Locale::from_code(loc).code() != loc.as_str() {
            return ApiError::bad_request("locale.unsupported").into_response(&req);
        }
    }
    // Validate display_name length when provided. Empty / whitespace-only
    // is rejected; over 80 chars is rejected to match workspace-name rules.
    if let Some(name) = &body.display_name {
        let trimmed = name.trim();
        if trimmed.is_empty() || trimmed.len() > 80 {
            return ApiError::bad_request("profile.display_name_invalid").into_response(&req);
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

// ── PATCH /api/w/:slug/settings/name ──────────────────────────────────────────

#[derive(Deserialize, Validate)]
pub struct UpdateWorkspaceName {
    name: String,
}

pub async fn update_workspace_name(
    req: Request,
    ctx: RouteContext<()>,
    a: Admin,
    body: UpdateWorkspaceName,
) -> Result<Response> {
    let slug = a.ws.slug;
    let index_db = crate::db::get_index_db(&ctx.env)?;

    let trimmed = body.name.trim();
    if trimmed.is_empty() || trimmed.len() > 80 {
        return ApiError::bad_request("workspace.name_invalid").into_response(&req);
    }

    crate::db::update_workspace_name(&index_db, &slug, trimmed).await?;

    let mut resp = Response::from_json(&serde_json::json!({ "ok": true }))?;
    let origin = req.headers().get("Origin")?.unwrap_or_default();
    middleware::add_cors(&mut resp, Some(&origin))?;
    Ok(resp)
}
