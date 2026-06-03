//! REST routes for events (workspace-scoped, JWT-auth).

use crate::extract::{ApiError, Member};
use crate::routes::util::read_query;
use crate::{d1_rest, db, middleware};
use serde::Deserialize;
use validator::Validate;
use worker::*;

/// Normalize an all-day event's instant to its calendar date at UTC midnight,
/// so the storage layer writes a bare "YYYY-MM-DD". The date-picker value is
/// taken literally (no tz conversion → no day shift).
fn utc_midnight(d: chrono::DateTime<chrono::Utc>) -> chrono::DateTime<chrono::Utc> {
    use chrono::TimeZone;
    chrono::Utc.from_utc_datetime(&d.date_naive().and_hms_opt(0, 0, 0).unwrap())
}

// ── GET /api/w/:slug/events ───────────────────────────────────────────────────

pub async fn list(req: Request, ctx: RouteContext<()>, m: Member) -> Result<Response> {
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
    let ws_db = db::WorkspaceDb::new(&client, m.ws.d1_database_id);
    let events = ws_db.list_events_in_range(&from, &to).await?;

    middleware::with_cors(&req, Response::from_json(&events)?)
}

// ── POST /api/w/:slug/events ──────────────────────────────────────────────────

#[derive(Deserialize, Validate)]
pub struct CreateBody {
    #[validate(length(min = 1, code = "event.title_required"))]
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

pub async fn create(
    req: Request,
    ctx: RouteContext<()>,
    m: Member,
    body: CreateBody,
) -> Result<Response> {
    // Cross-field order check — kept here rather than in the DTO since
    // `validator` 0.18 schema-level validation is clumsier than a guard line.
    if let Some(ends_at) = body.ends_at {
        if ends_at < body.starts_at {
            return ApiError::bad_request("event.range_invalid").into_response(&req);
        }
    }

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, m.ws.d1_database_id);

    // events.created_by is FK → members(id). Resolve the calling user's
    // member-id in this workspace via their Telegram numeric id; if they
    // aren't a member yet, fall back to NULL rather than failing FK.
    let created_by = match m.claims.tg_user_id.as_deref() {
        Some(tg) if !tg.is_empty() => ws_db.find_member_by_platform_id(tg).await.ok().flatten(),
        _ => None,
    };

    // All-day events are civil dates: normalize to UTC-midnight so the DB layer
    // writes a bare date (no time component, no day shift).
    let (starts_at, ends_at) = if body.all_day {
        (utc_midnight(body.starts_at), body.ends_at.map(utc_midnight))
    } else {
        (body.starts_at, body.ends_at)
    };

    let new = grumps_calendar::NewEvent {
        title: body.title,
        description: body.description,
        starts_at,
        ends_at,
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

pub async fn get(req: Request, ctx: RouteContext<()>, m: Member) -> Result<Response> {
    let id = ctx
        .param("id")
        .ok_or_else(|| Error::RustError("missing id".into()))?
        .to_string();

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, m.ws.d1_database_id);

    match ws_db.get_event(&id).await? {
        Some(e) => middleware::with_cors(&req, Response::from_json(&e)?),
        None => ApiError::not_found("event.not_found").into_response(&req),
    }
}

// ── PUT /api/w/:slug/events/:id ───────────────────────────────────────────────

#[derive(Deserialize, Validate)]
pub struct UpdateBody {
    title: Option<String>,
    starts_at: Option<String>,
    ends_at: Option<String>,
    location: Option<String>,
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

    // Validate range when both ends are provided. ISO-8601 / RFC3339 strings
    // sort lexically — sufficient for an order check on UTC-normalized input.
    if let (Some(starts), Some(ends)) = (&body.starts_at, &body.ends_at) {
        if ends < starts {
            return ApiError::bad_request("event.range_invalid").into_response(&req);
        }
    }

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, m.ws.d1_database_id);

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
        return ApiError::not_found("event.not_found").into_response(&req);
    }

    let event = ws_db.get_event(&id).await?;
    middleware::with_cors(&req, Response::from_json(&event)?)
}

// ── DELETE /api/w/:slug/events/:id ────────────────────────────────────────────

pub async fn delete(req: Request, ctx: RouteContext<()>, m: Member) -> Result<Response> {
    let id = ctx
        .param("id")
        .ok_or_else(|| Error::RustError("missing id".into()))?
        .to_string();

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, m.ws.d1_database_id);

    let deleted = ws_db.delete_event(&id).await?;
    if !deleted {
        return ApiError::not_found("event.not_found").into_response(&req);
    }

    middleware::with_cors(&req, Response::empty()?.with_status(204))
}
