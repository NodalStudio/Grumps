//! REST routes for scheduled_actions (workspace-scoped, JWT-auth).

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

// ── GET /api/w/:slug/scheduled ────────────────────────────────────────────────

pub async fn list(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = auth(&req, &ctx)?;
    let ws = resolve_workspace(&ctx).await?;
    access(&claims, &ws.slug)?;

    let url = req.url()?;
    let mut status: Option<String> = None;
    let mut limit = 50i64;
    let mut offset = 0i64;
    read_query(&url, |k, v| match k {
        "status" => status = Some(v.to_string()),
        "limit"  => { limit  = v.parse().unwrap_or(50).clamp(1, 200); }
        "offset" => { offset = v.parse().unwrap_or(0).max(0); }
        _ => {}
    });

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);
    let actions = ws_db.list_scheduled_actions(status.as_deref(), limit, offset).await?;

    middleware::with_cors(&req, Response::from_json(&actions)?)
}

// ── POST /api/w/:slug/scheduled ───────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateBody {
    action_type: grumps_scheduler::ActionType,
    title: String,
    trigger_at: chrono::DateTime<chrono::Utc>,
    recurrence: Option<String>,
    condition: Option<serde_json::Value>,
    payload: serde_json::Value,
}

pub async fn create(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = auth(&req, &ctx)?;
    let ws = resolve_workspace(&ctx).await?;
    access(&claims, &ws.slug)?;

    let slug = ws.slug.clone();
    let body: CreateBody = req.json().await
        .map_err(|e| Error::RustError(format!("bad body: {e}")))?;

    let trigger_at_iso = body.trigger_at.to_rfc3339();

    let new = grumps_scheduler::NewScheduledAction {
        action_type: body.action_type,
        title: body.title,
        trigger_at: body.trigger_at,
        recurrence: body.recurrence,
        condition: body.condition,
        payload: body.payload,
        target_chat: Some("group".into()),
        created_by: Some(claims.sub.clone()),
    };

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);

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
            }))?.with_status(503),
        );
    }

    // 4. Fetch and return the created action
    let action = ws_db.get_scheduled_action(&id).await?;
    middleware::with_cors(&req, Response::from_json(&action)?.with_status(201))
}

// ── GET /api/w/:slug/scheduled/:id ───────────────────────────────────────────

pub async fn get(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = auth(&req, &ctx)?;
    let ws = resolve_workspace(&ctx).await?;
    access(&claims, &ws.slug)?;

    let id = ctx.param("id").ok_or_else(|| Error::RustError("missing id".into()))?.to_string();

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);

    match ws_db.get_scheduled_action(&id).await? {
        Some(a) => middleware::with_cors(&req, Response::from_json(&a)?),
        None    => middleware::with_cors(&req, Response::error("scheduled action not found", 404)?),
    }
}

// ── DELETE /api/w/:slug/scheduled/:id ────────────────────────────────────────

pub async fn delete(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = auth(&req, &ctx)?;
    let ws = resolve_workspace(&ctx).await?;
    access(&claims, &ws.slug)?;

    let slug = ws.slug.clone();
    let id = ctx.param("id").ok_or_else(|| Error::RustError("missing id".into()))?.to_string();

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id);

    let deleted = ws_db.delete_scheduled_action(&id).await?;
    if !deleted {
        return middleware::with_cors(&req, Response::error("scheduled action not found", 404)?);
    }

    // Best-effort: tell DO to recompute its next alarm
    let _ = reschedule_do(&ctx.env, &slug).await;

    middleware::with_cors(&req, Response::empty()?.with_status(204))
}

// ── DO RPC helpers ────────────────────────────────────────────────────────────

/// RPC the DO to arm a new alarm. Retries 3x.
async fn arm_do_alarm(env: &Env, slug: &str, trigger_at_iso: &str) -> Result<()> {
    let do_ns = env.durable_object("WS_SCHEDULER")?;
    let id = do_ns.id_from_name(slug)?;
    let stub = id.get_stub()?;
    let body = serde_json::json!({ "op": "schedule", "trigger_at": trigger_at_iso });

    let mut attempts = 0;
    loop {
        attempts += 1;
        // Build a fresh request each iteration (Request is not Clone)
        let mut headers = Headers::new();
        headers.set("content-type", "application/json")?;
        let req = Request::new_with_init(
            "https://do/",
            RequestInit::new()
                .with_method(Method::Post)
                .with_headers(headers)
                .with_body(Some(serde_json::to_string(&body).unwrap().into())),
        )?;
        match stub.fetch_with_request(req).await {
            Ok(resp) if resp.status_code() < 500 => return Ok(()),
            Ok(_) | Err(_) if attempts >= 3 => {
                return Err(Error::RustError("DO RPC failed after 3 retries".into()))
            }
            _ => { /* retry */ }
        }
    }
}

/// Tell DO to recompute its next alarm (best-effort).
async fn reschedule_do(env: &Env, slug: &str) -> Result<()> {
    let do_ns = env.durable_object("WS_SCHEDULER")?;
    let id = do_ns.id_from_name(slug)?;
    let stub = id.get_stub()?;
    let body = serde_json::json!({ "op": "reschedule" });
    let mut headers = Headers::new();
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
