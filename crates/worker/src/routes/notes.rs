// crates/worker/src/routes/notes.rs
use worker::*;
use serde::Deserialize;
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

// ── GET /api/w/:slug/notes ────────────────────────────────────────────────────

pub async fn list_notes(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = auth(&req, &ctx)?;
    let ws = resolve_workspace(&ctx).await?;
    access(&claims, &ws.slug)?;

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);
    let notes = ws_db.get_notes().await?;

    let data: Vec<serde_json::Value> = notes.iter().map(|(id, title, source, created_at)| {
        serde_json::json!({
            "id": id,
            "title": title,
            "source": source,
            "created_at": created_at,
        })
    }).collect();

    middleware::with_cors(&req, Response::from_json(&data)?)
}

// ── POST /api/w/:slug/notes ───────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateNote {
    title: Option<String>,
    content: String,
}

pub async fn create_note(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = auth(&req, &ctx)?;
    let ws = resolve_workspace(&ctx).await?;
    access(&claims, &ws.slug)?;

    let body: CreateNote = req.json().await.map_err(|_| Error::RustError("invalid json".into()))?;

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);

    let note_id = ws_db.insert_note(
        &body.title.unwrap_or_default(),
        &body.content,
        "api",
        &claims.sub,
    ).await?;

    middleware::with_cors(&req, Response::from_json(&serde_json::json!({ "id": note_id }))?)
}

// ── GET /api/w/:slug/notes/:id ────────────────────────────────────────────────

pub async fn get_note(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = auth(&req, &ctx)?;
    let ws = resolve_workspace(&ctx).await?;
    access(&claims, &ws.slug)?;

    let note_id = ctx.param("id").ok_or_else(|| Error::RustError("missing id".into()))?.to_string();

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);

    match ws_db.get_note_by_id(&note_id).await? {
        Some(note) => middleware::with_cors(&req, Response::from_json(&note)?),
        None => middleware::with_cors(&req, Response::error("note not found", 404)?),
    }
}

// ── PUT /api/w/:slug/notes/:id ────────────────────────────────────────────────

#[derive(Deserialize)]
struct UpdateNote {
    title: Option<String>,
    content: String,
}

pub async fn update_note(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = auth(&req, &ctx)?;
    let ws = resolve_workspace(&ctx).await?;
    access(&claims, &ws.slug)?;

    let note_id = ctx.param("id").ok_or_else(|| Error::RustError("missing id".into()))?.to_string();
    let body: UpdateNote = req.json().await.map_err(|_| Error::RustError("invalid json".into()))?;

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);

    ws_db.update_note(
        &note_id,
        &body.title.unwrap_or_default(),
        &body.content,
        &claims.sub,
    ).await?;

    middleware::with_cors(&req, Response::from_json(&serde_json::json!({ "ok": true }))?)
}

// ── DELETE /api/w/:slug/notes/:id ────────────────────────────────────────────

pub async fn delete_note(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = auth(&req, &ctx)?;
    let ws = resolve_workspace(&ctx).await?;
    access(&claims, &ws.slug)?;

    let note_id = ctx.param("id").ok_or_else(|| Error::RustError("missing id".into()))?.to_string();

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);
    ws_db.delete_note(&note_id).await?;

    middleware::with_cors(&req, Response::from_json(&serde_json::json!({ "ok": true }))?)
}
