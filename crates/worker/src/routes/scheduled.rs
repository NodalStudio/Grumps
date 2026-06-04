//! REST routes for scheduled_actions (workspace-scoped, JWT-auth).

use crate::extract::{ApiError, Member};
use crate::routes::util::read_query;
use crate::{d1_rest, db, middleware};
use serde::Deserialize;
use validator::Validate;
use worker::*;

// ── GET /api/w/:slug/scheduled ────────────────────────────────────────────────

pub async fn list(req: Request, ctx: RouteContext<()>, m: Member) -> Result<Response> {
    let url = req.url()?;
    let mut status: Option<String> = None;
    let mut limit = 50i64;
    let mut offset = 0i64;
    read_query(&url, |k, v| match k {
        "status" => status = Some(v.to_string()),
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
    let actions = ws_db
        .list_scheduled_actions(status.as_deref(), limit, offset)
        .await?;

    middleware::with_cors(&req, Response::from_json(&actions)?)
}

// ── POST /api/w/:slug/scheduled ───────────────────────────────────────────────

#[derive(Deserialize, Validate)]
pub struct CreateBody {
    action_type: grumps_scheduler::ActionType,
    #[serde(deserialize_with = "grumps_core::dto::de_trim")]
    #[validate(length(min = 1, code = "schedule.title_required"))]
    title: String,
    trigger_at: chrono::DateTime<chrono::Utc>,
    recurrence: Option<String>,
    condition: Option<serde_json::Value>,
    payload: serde_json::Value,
}

pub async fn create(
    req: Request,
    ctx: RouteContext<()>,
    m: Member,
    body: CreateBody,
) -> Result<Response> {
    let slug = m.ws.slug.clone();

    // Reject past triggers (more than 60s in the past) — non-recurring
    // past triggers would never fire; recurring ones could fire every
    // tick until the runtime catches up. The 60s grace window absorbs
    // clock skew between client and Worker.
    let now = chrono::Utc::now();
    if body.trigger_at < now - chrono::Duration::seconds(60) {
        return ApiError::bad_request("schedule.trigger_in_past").into_response(&req);
    }

    let trigger_at_iso = body.trigger_at.to_rfc3339();

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, m.ws.d1_database_id);

    // scheduled_actions.created_by is FK → members(id).
    let created_by = match m.claims.tg_user_id.as_deref() {
        Some(tg) if !tg.is_empty() => ws_db.find_member_by_platform_id(tg).await.ok().flatten(),
        _ => None,
    };

    let new = grumps_scheduler::NewScheduledAction {
        action_type: body.action_type,
        title: body.title,
        trigger_at: body.trigger_at,
        recurrence: body.recurrence,
        condition: body.condition,
        payload: body.payload,
        target_chat: Some("group".into()),
        created_by,
    };

    // 1. INSERT into D1
    let id = ws_db.create_scheduled_action(&new).await?;

    // 2. RPC the DO to arm the alarm
    if let Err(e) = arm_do_alarm(&ctx.env, &slug, &trigger_at_iso).await {
        // 3. Rollback: delete the D1 row
        let _ = ws_db.delete_scheduled_action(&id).await;
        return middleware::with_cors(
            &req,
            Response::from_json(&serde_json::json!({
                "error": "scheduling_failed",
                "message": format!("DO RPC failed: {e}")
            }))?
            .with_status(503),
        );
    }

    // 4. Fetch and return the created action
    let action = ws_db.get_scheduled_action(&id).await?;
    middleware::with_cors(&req, Response::from_json(&action)?.with_status(201))
}

// ── GET /api/w/:slug/scheduled/:id ───────────────────────────────────────────

pub async fn get(req: Request, ctx: RouteContext<()>, m: Member) -> Result<Response> {
    let id = ctx
        .param("id")
        .ok_or_else(|| Error::RustError("missing id".into()))?
        .to_string();

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, m.ws.d1_database_id);

    match ws_db.get_scheduled_action(&id).await? {
        Some(a) => middleware::with_cors(&req, Response::from_json(&a)?),
        None => ApiError::not_found("schedule.not_found").into_response(&req),
    }
}

// ── DELETE /api/w/:slug/scheduled/:id ────────────────────────────────────────

pub async fn delete(req: Request, ctx: RouteContext<()>, m: Member) -> Result<Response> {
    let slug = m.ws.slug.clone();
    let id = ctx
        .param("id")
        .ok_or_else(|| Error::RustError("missing id".into()))?
        .to_string();

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, m.ws.d1_database_id);

    let deleted = ws_db.delete_scheduled_action(&id).await?;
    if !deleted {
        return ApiError::not_found("schedule.not_found").into_response(&req);
    }

    // Best-effort: tell DO to recompute its next alarm
    let _ = reschedule_do(&ctx.env, &slug).await;

    middleware::with_cors(&req, Response::empty()?.with_status(204))
}

// ── DO RPC helpers ────────────────────────────────────────────────────────────

/// RPC the DO to arm a new alarm. Retries 3x.
pub(crate) async fn arm_do_alarm(env: &Env, slug: &str, trigger_at_iso: &str) -> Result<()> {
    let do_ns = env.durable_object("WS_SCHEDULER")?;
    let id = do_ns.id_from_name(slug)?;
    let stub = id.get_stub()?;
    let body = serde_json::json!({ "op": "schedule", "trigger_at": trigger_at_iso });

    let mut attempts = 0;
    loop {
        attempts += 1;
        // Build a fresh request each iteration (Request is not Clone)
        let headers = Headers::new();
        headers.set("content-type", "application/json")?;
        let req = Request::new_with_init(
            "https://do/",
            RequestInit::new()
                .with_method(Method::Post)
                .with_headers(headers)
                .with_body(Some(serde_json::to_string(&body).unwrap().into())),
        )?;
        match stub.fetch_with_request(req).await {
            // Only 2xx means the DO actually armed the alarm.
            Ok(resp) if (200..300).contains(&resp.status_code()) => return Ok(()),
            // 4xx is a non-retryable rejection — fail loudly rather than
            // reporting a success the DO never granted.
            Ok(resp) if (400..500).contains(&resp.status_code()) => {
                return Err(Error::RustError(format!(
                    "DO alarm rejected with status {}",
                    resp.status_code()
                )))
            }
            // 5xx / transport error: retry up to 3 times, then give up.
            Ok(_) | Err(_) if attempts >= 3 => {
                return Err(Error::RustError("DO RPC failed after 3 retries".into()))
            }
            _ => { /* retry */ }
        }
    }
}

/// Tell DO to recompute its next alarm (best-effort).
pub(crate) async fn reschedule_do(env: &Env, slug: &str) -> Result<()> {
    let do_ns = env.durable_object("WS_SCHEDULER")?;
    let id = do_ns.id_from_name(slug)?;
    let stub = id.get_stub()?;
    let body = serde_json::json!({ "op": "reschedule" });
    let headers = Headers::new();
    headers.set("content-type", "application/json")?;
    let req = Request::new_with_init(
        "https://do/",
        RequestInit::new()
            .with_method(Method::Post)
            .with_headers(headers)
            .with_body(Some(serde_json::to_string(&body).unwrap().into())),
    )?;
    let _ = stub.fetch_with_request(req).await;
    Ok(())
}
