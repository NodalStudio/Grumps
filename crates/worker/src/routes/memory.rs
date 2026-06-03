//! REST routes for memory_entries (workspace-scoped, JWT-auth).

use crate::extract::Member;
use crate::routes::util::read_query;
use crate::{d1_rest, db, middleware};
use serde::Deserialize;
use validator::Validate;
use worker::*;

// ── GET /api/w/:slug/memory ───────────────────────────────────────────────────

pub async fn list(req: Request, ctx: RouteContext<()>, m: Member) -> Result<Response> {
    let url = req.url()?;
    let mut kind: Option<String> = None;
    let mut source: Option<String> = None;
    let mut limit = 50i64;
    let mut offset = 0i64;
    read_query(&url, |k, v| match k {
        "kind" => kind = Some(v.to_string()),
        "source" => source = Some(v.to_string()),
        "limit" => {
            limit = v.parse().unwrap_or(50).clamp(1, 200);
        }
        "offset" => {
            offset = v.parse().unwrap_or(0).max(0);
        }
        _ => {}
    });

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, m.ws.d1_database_id);
    let entries = ws_db
        .list_memory(kind.as_deref(), source.as_deref(), limit, offset)
        .await?;

    middleware::with_cors(&req, Response::from_json(&entries)?)
}

// ── POST /api/w/:slug/memory ──────────────────────────────────────────────────

#[derive(Deserialize, Validate)]
pub struct CreateBody {
    key: Option<String>,
    #[validate(length(min = 1, code = "memory.value_required"))]
    value: String,
    kind: grumps_memory::MemoryKind,
    related_member: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    pinned: Option<bool>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn create(
    req: Request,
    ctx: RouteContext<()>,
    m: Member,
    body: CreateBody,
) -> Result<Response> {
    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, m.ws.d1_database_id);

    // memory_entries.created_by is FK → members(id). Resolve the calling
    // user's member-id in this workspace via their Telegram numeric id; if
    // they aren't a member yet, fall back to NULL rather than failing FK.
    let created_by = match m.claims.tg_user_id.as_deref() {
        Some(tg) if !tg.is_empty() => ws_db.find_member_by_platform_id(tg).await.ok().flatten(),
        _ => None,
    };

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
        created_by,
    };
    let id = ws_db.create_memory(&new).await?;
    let entry = ws_db.get_memory(&id).await?;

    middleware::with_cors(&req, Response::from_json(&entry)?.with_status(201))
}

// ── GET /api/w/:slug/memory/:id ───────────────────────────────────────────────

pub async fn get(req: Request, ctx: RouteContext<()>, m: Member) -> Result<Response> {
    let id = ctx
        .param("id")
        .ok_or_else(|| Error::RustError("missing id".into()))?
        .to_string();

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, m.ws.d1_database_id);

    match ws_db.get_memory(&id).await? {
        Some(e) => middleware::with_cors(&req, Response::from_json(&e)?),
        None => middleware::with_cors(&req, Response::error("memory entry not found", 404)?),
    }
}

// ── PUT /api/w/:slug/memory/:id ───────────────────────────────────────────────

#[derive(Deserialize, Validate)]
pub struct UpdateBody {
    value: Option<String>,
    pinned: Option<bool>,
    expires_at: Option<String>,
}

pub async fn update(
    req: Request,
    ctx: RouteContext<()>,
    m: Member,
    body: UpdateBody,
) -> Result<Response> {
    let id = ctx
        .param("id")
        .ok_or_else(|| Error::RustError("missing id".into()))?
        .to_string();

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, m.ws.d1_database_id);

    let updated = ws_db
        .update_memory(
            &id,
            body.value.as_deref(),
            body.pinned,
            body.expires_at.as_deref(),
        )
        .await?;
    if !updated {
        return middleware::with_cors(&req, Response::error("memory entry not found", 404)?);
    }

    let entry = ws_db.get_memory(&id).await?;
    middleware::with_cors(&req, Response::from_json(&entry)?)
}

// ── DELETE /api/w/:slug/memory/:id ────────────────────────────────────────────

pub async fn delete(req: Request, ctx: RouteContext<()>, m: Member) -> Result<Response> {
    let id = ctx
        .param("id")
        .ok_or_else(|| Error::RustError("missing id".into()))?
        .to_string();

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, m.ws.d1_database_id);

    let deleted = ws_db.delete_memory(&id).await?;
    if !deleted {
        return middleware::with_cors(&req, Response::error("memory entry not found", 404)?);
    }

    middleware::with_cors(&req, Response::empty()?.with_status(204))
}
