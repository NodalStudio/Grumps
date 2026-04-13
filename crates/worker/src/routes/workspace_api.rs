// crates/worker/src/routes/workspace_api.rs
use worker::*;
use crate::{db, middleware, d1_rest};

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
