//! REST routes for events (workspace-scoped, JWT-auth).

use crate::routes::util::read_query;
use crate::{d1_rest, db, middleware};
use serde::Deserialize;
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

// ── GET /api/w/:slug/events ───────────────────────────────────────────────────

pub async fn list(req: Request, ctx: RouteContext<()>) -> Result<Response> {
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
    let now = chrono::Utc::now();
    let default_from = (now - chrono::Duration::days(30)).to_rfc3339();
    let default_to = (now + chrono::Duration::days(60)).to_rfc3339();
    let mut from = default_from;
    let mut to = default_to;
    read_query(&url, |k, v| match k {
        "from" => from = v.to_string(),
        "to" => to = v.to_string(),
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

    let body: CreateBody = match req.json().await {
        Ok(b) => b,
        Err(_) => return middleware::error_with_cors(&req, 400, "bad_request", "invalid JSON"),
    };

    if body.title.trim().is_empty() {
        return middleware::error_with_cors(&req, 400, "bad_request", "title required");
    }
    if let Some(ends_at) = body.ends_at {
        if ends_at < body.starts_at {
            return middleware::error_with_cors(
                &req,
                400,
                "bad_request",
                "ends_at must be at or after starts_at",
            );
        }
    }

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);

    // events.created_by is FK → members(id). Resolve the calling user's
    // member-id in this workspace via their Telegram numeric id; if they
    // aren't a member yet, fall back to NULL rather than failing FK.
    let created_by = match claims.tg_user_id.as_deref() {
        Some(tg) if !tg.is_empty() => ws_db.find_member_by_platform_id(tg).await.ok().flatten(),
        _ => None,
    };

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
        created_by,
    };
    let id = ws_db.create_event(&new).await?;
    let event = ws_db.get_event(&id).await?;

    middleware::with_cors(&req, Response::from_json(&event)?.with_status(201))
}

// ── GET /api/w/:slug/events/:id ───────────────────────────────────────────────

pub async fn get(req: Request, ctx: RouteContext<()>) -> Result<Response> {
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

    let id = ctx
        .param("id")
        .ok_or_else(|| Error::RustError("missing id".into()))?
        .to_string();

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);

    match ws_db.get_event(&id).await? {
        Some(e) => middleware::with_cors(&req, Response::from_json(&e)?),
        None => middleware::with_cors(&req, Response::error("event not found", 404)?),
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

    let id = ctx
        .param("id")
        .ok_or_else(|| Error::RustError("missing id".into()))?
        .to_string();
    let body: UpdateBody = match req.json().await {
        Ok(b) => b,
        Err(_) => return middleware::error_with_cors(&req, 400, "bad_request", "invalid JSON"),
    };

    // Validate range when both ends are provided. ISO-8601 / RFC3339 strings
    // sort lexically — sufficient for an order check on UTC-normalized input.
    if let (Some(starts), Some(ends)) = (&body.starts_at, &body.ends_at) {
        if ends < starts {
            return middleware::error_with_cors(
                &req,
                400,
                "bad_request",
                "ends_at must be at or after starts_at",
            );
        }
    }

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);

    let updated = ws_db
        .update_event(
            &id,
            body.title.as_deref(),
            body.starts_at.as_deref(),
            body.ends_at.as_deref(),
            body.location.as_deref(),
        )
        .await?;

    if !updated {
        return middleware::with_cors(&req, Response::error("event not found", 404)?);
    }

    let event = ws_db.get_event(&id).await?;
    middleware::with_cors(&req, Response::from_json(&event)?)
}

// ── DELETE /api/w/:slug/events/:id ────────────────────────────────────────────

pub async fn delete(req: Request, ctx: RouteContext<()>) -> Result<Response> {
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

    let id = ctx
        .param("id")
        .ok_or_else(|| Error::RustError("missing id".into()))?
        .to_string();

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);

    let deleted = ws_db.delete_event(&id).await?;
    if !deleted {
        return middleware::with_cors(&req, Response::error("event not found", 404)?);
    }

    middleware::with_cors(&req, Response::empty()?.with_status(204))
}
