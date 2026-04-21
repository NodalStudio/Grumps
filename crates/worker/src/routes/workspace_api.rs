// crates/worker/src/routes/workspace_api.rs
use worker::*;
use crate::{db, middleware, d1_rest};
use grumps_messaging::telegram::TelegramAdapter;

// ── helpers ──────────────────────────────────────────────────────────────────

async fn resolve_workspace(ctx: &RouteContext<()>) -> Result<db::WorkspaceMetaRow> {
    let slug = ctx.param("slug").ok_or_else(|| Error::RustError("missing slug".into()))?;
    let index_db = db::get_index_db(&ctx.env)?;
    db::lookup_workspace_by_slug(&index_db, slug).await?
        .ok_or_else(|| Error::RustError("workspace not found".into()))
}

fn auth(req: &Request, ctx: &RouteContext<()>) -> Result<middleware::Claims> {
    let jwt_secret = ctx.env.secret("JWT_SECRET")?.to_string();
    middleware::verify_jwt(req, &jwt_secret).map_err(|e| Error::RustError(e))
}

fn access(claims: &middleware::Claims, slug: &str) -> Result<()> {
    middleware::check_workspace_access(claims, slug).map_err(|e| Error::RustError(e))
}

// ── GET /api/workspaces ───────────────────────────────────────────────────────

pub async fn list_my_workspaces(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = auth(&req, &ctx)?;

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
    let claims = auth(&req, &ctx)?;
    let ws = resolve_workspace(&ctx).await?;
    access(&claims, &ws.slug)?;

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);
    let (open, done_week, notes, _files) = ws_db.get_status_counts().await?;

    middleware::with_cors(&req, Response::from_json(&serde_json::json!({
        "slug": ws.slug,
        "name": ws.name,
        "plan": ws.plan,
        "stats": {
            "open_todos": open,
            "done_this_week": done_week,
            "notes": notes,
        }
    }))?)
}

// ── GET /api/w/:slug/history ──────────────────────────────────────────────────

pub async fn workspace_history(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = auth(&req, &ctx)?;
    let ws = resolve_workspace(&ctx).await?;
    access(&claims, &ws.slug)?;

    let url = req.url()?;
    let params: std::collections::HashMap<String, String> = url.query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let limit: i64 = params.get("limit")
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
    let claims = auth(&req, &ctx)?;
    let ws = resolve_workspace(&ctx).await?;
    access(&claims, &ws.slug)?;

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);
    let members = ws_db.get_members().await?;

    middleware::with_cors(&req, Response::from_json(&members)?)
}

// ── PATCH /api/w/:slug/settings/locale ────────────────────────────────────────

pub async fn update_locale(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = auth(&req, &ctx)?;
    let ws = resolve_workspace(&ctx).await?;
    access(&claims, &ws.slug)?;

    let index_db = db::get_index_db(&ctx.env)?;

    // Workspace-admin-only.
    let is_admin = middleware::is_workspace_admin_by_slug(&index_db, &claims.sub, &ws.slug).await
        .unwrap_or(false);
    if !is_admin {
        return middleware::with_cors(&req, Response::error("forbidden: admin required", 403)?);
    }

    #[derive(serde::Deserialize)]
    struct Body { locale: String }
    let body: Body = req.json().await
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
    if let Ok(Some((platform, channel_id))) = db::lookup_platform_channel(&index_db, &ws.slug).await {
        if platform == "telegram" {
            let tg = build_tg_adapter(&ctx)?;
            let desc = grumps_i18n::t(resolved, "telegram.onboarding.description", &[("slug", &ws.slug)]);
            let _ = crate::routes::webhook_telegram::call_set_description(&tg, &channel_id, &desc).await;
        }
    }

    middleware::with_cors(&req, Response::from_json(&serde_json::json!({
        "ok": true,
        "locale": resolved.code()
    }))?)
}

fn build_tg_adapter(ctx: &RouteContext<()>) -> Result<TelegramAdapter> {
    Ok(TelegramAdapter::new(
        ctx.env.secret("TG_BOT_TOKEN")?.to_string(),
        ctx.env.var("TG_BOT_USERNAME")?.to_string(),
        ctx.env.secret("TG_WEBHOOK_SECRET")?.to_string(),
    ))
}
