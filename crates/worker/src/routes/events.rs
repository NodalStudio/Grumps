//! REST routes for events (workspace-scoped, JWT-auth).

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

fn auth(req: &Request, ctx: &RouteContext<()>) -> Result<middleware::Claims> {
    let jwt_secret = ctx.env.secret("JWT_SECRET")?.to_string();
    middleware::verify_jwt(req, &jwt_secret).map_err(|e| Error::RustError(e))
}

fn access(claims: &middleware::Claims, slug: &str) -> Result<()> {
    middleware::check_workspace_access(claims, slug).map_err(|e| Error::RustError(e))
}

// ── GET /api/w/:slug/events ───────────────────────────────────────────────────

pub async fn list(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = auth(&req, &ctx)?;
    let ws = resolve_workspace(&ctx).await?;
    access(&claims, &ws.slug)?;

    let url = req.url()?;
    let now = chrono::Utc::now();
    let default_from = (now - chrono::Duration::days(30)).to_rfc3339();
    let default_to   = (now + chrono::Duration::days(60)).to_rfc3339();
    let mut from = default_from;
    let mut to   = default_to;
    read_query(&url, |k, v| match k {
        "from" => from = v.to_string(),
        "to"   => to   = v.to_string(),
        _ => {}
    });

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);
    let events = ws_db.list_events_in_range(&from, &to).await?;

    middleware::with_cors(&req, Response::from_json(&events)?)
}

// ── POST /api/w/:slug/events ──────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateBody {
    title: String,
    description: Option<String>,
    starts_at: chrono::DateTime<chrono::Utc>,
    ends_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    all_day: bool,
    location: Option<String>,
    recurrence: Option<String>,
    #[serde(default)]
    attendees: Vec<String>,
    color: Option<String>,
}

pub async fn create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = auth(&req, &ctx)?;
    let ws = resolve_workspace(&ctx).await?;
    access(&claims, &ws.slug)?;

    let body: CreateBody = req.json().await
        .map_err(|e| Error::RustError(format!("bad body: {e}")))?;

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);

    let new = grumps_calendar::NewEvent {
        title: body.title,
        description: body.description,
        starts_at: body.starts_at,
        ends_at: body.ends_at,
        all_day: body.all_day,
        location: body.location,
        recurrence: body.recurrence,
        attendees: body.attendees,
        color: body.color,
        source: grumps_calendar::EventSource::Web,
        related_todo_id: None,
        created_by: Some(claims.sub.clone()),
    };
    let id = ws_db.create_event(&new).await?;
    let event = ws_db.get_event(&id).await?;

    middleware::with_cors(&req, Response::from_json(&event)?.with_status(201))
}

// ── GET /api/w/:slug/events/:id ───────────────────────────────────────────────

pub async fn get(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = auth(&req, &ctx)?;
    let ws = resolve_workspace(&ctx).await?;
    access(&claims, &ws.slug)?;

    let id = ctx.param("id").ok_or_else(|| Error::RustError("missing id".into()))?.to_string();

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);

    match ws_db.get_event(&id).await? {
        Some(e) => middleware::with_cors(&req, Response::from_json(&e)?),
        None    => middleware::with_cors(&req, Response::error("event not found", 404)?),
    }
}

// ── PUT /api/w/:slug/events/:id ───────────────────────────────────────────────

#[derive(Deserialize)]
struct UpdateBody {
    title: Option<String>,
    starts_at: Option<String>,
    ends_at: Option<String>,
    location: Option<String>,
}

pub async fn update(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = auth(&req, &ctx)?;
    let ws = resolve_workspace(&ctx).await?;
    access(&claims, &ws.slug)?;

    let id = ctx.param("id").ok_or_else(|| Error::RustError("missing id".into()))?.to_string();
    let body: UpdateBody = req.json().await
        .map_err(|e| Error::RustError(format!("bad body: {e}")))?;

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);

    let updated = ws_db.update_event(
        &id,
        body.title.as_deref(),
        body.starts_at.as_deref(),
        body.ends_at.as_deref(),
        body.location.as_deref(),
    ).await?;

    if !updated {
        return middleware::with_cors(&req, Response::error("event not found", 404)?);
    }

    let event = ws_db.get_event(&id).await?;
    middleware::with_cors(&req, Response::from_json(&event)?)
}

// ── DELETE /api/w/:slug/events/:id ────────────────────────────────────────────

pub async fn delete(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = auth(&req, &ctx)?;
    let ws = resolve_workspace(&ctx).await?;
    access(&claims, &ws.slug)?;

    let id = ctx.param("id").ok_or_else(|| Error::RustError("missing id".into()))?.to_string();

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);

    let deleted = ws_db.delete_event(&id).await?;
    if !deleted {
        return middleware::with_cors(&req, Response::error("event not found", 404)?);
    }

    middleware::with_cors(&req, Response::empty()?.with_status(204))
}
