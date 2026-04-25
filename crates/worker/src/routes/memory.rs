//! REST routes for memory_entries (workspace-scoped, JWT-auth).

use worker::*;
use serde::Deserialize;
use crate::{db, middleware, d1_rest};
use crate::routes::util::read_query;

// ── helpers ──────────────────────────────────────────────────────────────────

async fn resolve_workspace(ctx: &RouteContext<()>) -> Result<db::WorkspaceMetaRow> {
    let slug = ctx.param("slug").ok_or_else(|| Error::RustError("missing slug".into()))?;
    let index_db = db::get_index_db(&ctx.env)?;
    db::lookup_workspace_by_slug(&index_db, slug).await?
        .ok_or_else(|| Error::RustError("workspace not found".into()))
}

// ── GET /api/w/:slug/memory ───────────────────────────────────────────────────

pub async fn list(req: Request, ctx: RouteContext<()>) -> Result<Response> {
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

    let url = req.url()?;
    let mut kind: Option<String> = None;
    let mut source: Option<String> = None;
    let mut limit = 50i64;
    let mut offset = 0i64;
    read_query(&url, |k, v| match k {
        "kind"   => kind   = Some(v.to_string()),
        "source" => source = Some(v.to_string()),
        "limit"  => { limit  = v.parse().unwrap_or(50).clamp(1, 200); }
        "offset" => { offset = v.parse().unwrap_or(0).max(0); }
        _ => {}
    });

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);
    let entries = ws_db.list_memory(kind.as_deref(), source.as_deref(), limit, offset).await?;

    middleware::with_cors(&req, Response::from_json(&entries)?)
}

// ── POST /api/w/:slug/memory ──────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateBody {
    key: Option<String>,
    value: String,
    kind: grumps_memory::MemoryKind,
    related_member: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    pinned: Option<bool>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
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

    let body: CreateBody = req.json().await
        .map_err(|e| Error::RustError(format!("bad body: {e}")))?;

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);

    let new = grumps_memory::NewMemoryEntry {
        key: body.key,
        value: body.value,
        kind: body.kind,
        related_member: body.related_member,
        tags: body.tags,
        source: grumps_memory::MemorySource::Web,
        confidence: Some(1.0),
        pinned: body.pinned,
        expires_at: body.expires_at,
        created_by: Some(claims.sub.clone()),
    };
    let id = ws_db.create_memory(&new).await?;
    let entry = ws_db.get_memory(&id).await?;

    middleware::with_cors(&req, Response::from_json(&entry)?.with_status(201))
}

// ── GET /api/w/:slug/memory/:id ───────────────────────────────────────────────

pub async fn get(req: Request, ctx: RouteContext<()>) -> Result<Response> {
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

    let id = ctx.param("id").ok_or_else(|| Error::RustError("missing id".into()))?.to_string();

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);

    match ws_db.get_memory(&id).await? {
        Some(e) => middleware::with_cors(&req, Response::from_json(&e)?),
        None    => middleware::with_cors(&req, Response::error("memory entry not found", 404)?),
    }
}

// ── PUT /api/w/:slug/memory/:id ───────────────────────────────────────────────

#[derive(Deserialize)]
struct UpdateBody {
    value: Option<String>,
    pinned: Option<bool>,
    expires_at: Option<String>,
}

pub async fn update(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
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

    let id = ctx.param("id").ok_or_else(|| Error::RustError("missing id".into()))?.to_string();
    let body: UpdateBody = req.json().await
        .map_err(|e| Error::RustError(format!("bad body: {e}")))?;

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);

    let updated = ws_db.update_memory(&id, body.value.as_deref(), body.pinned, body.expires_at.as_deref()).await?;
    if !updated {
        return middleware::with_cors(&req, Response::error("memory entry not found", 404)?);
    }

    let entry = ws_db.get_memory(&id).await?;
    middleware::with_cors(&req, Response::from_json(&entry)?)
}

// ── DELETE /api/w/:slug/memory/:id ────────────────────────────────────────────

pub async fn delete(req: Request, ctx: RouteContext<()>) -> Result<Response> {
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

    let id = ctx.param("id").ok_or_else(|| Error::RustError("missing id".into()))?.to_string();

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);

    let deleted = ws_db.delete_memory(&id).await?;
    if !deleted {
        return middleware::with_cors(&req, Response::error("memory entry not found", 404)?);
    }

    middleware::with_cors(&req, Response::empty()?.with_status(204))
}
