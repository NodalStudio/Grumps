//! Cross-workspace aggregated endpoints (super admin only).

use crate::d1_rest::D1RestClient;
use crate::db::{get_index_db, WorkspaceDb};
use crate::middleware::{self, is_super_admin};
use serde::Serialize;
use worker::*;

#[derive(Serialize)]
pub struct GlobalObservability {
    pub generated_at: String,
    pub workspaces_count: usize,
    pub total_cost_usd: f64,
    pub total_calls: i64,
    pub by_workspace: Vec<WorkspaceStats>,
    pub cost_by_model: Vec<ModelCostAgg>,
    pub recent_errors: Vec<GlobalError>,
    pub quality_signals: Vec<grumps_agent::db::QualitySignalCount>,
}

#[derive(Serialize)]
pub struct WorkspaceStats {
    pub slug: String,
    pub name: Option<String>,
    pub plan: String,
    pub cost_usd: f64,
    pub calls: i64,
    pub quality_score: f64,
}

#[derive(Serialize)]
pub struct ModelCostAgg {
    pub provider: String,
    pub model: String,
    pub cost_usd: f64,
    pub call_count: i64,
}

#[derive(Serialize)]
pub struct GlobalError {
    pub workspace_slug: String,
    pub created_at: String,
    pub provider: String,
    pub model: String,
    pub error: String,
}

pub async fn observability(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match middleware::verify_session(&req, &ctx.env).await {
        Ok(c) => c,
        Err(e) => return middleware::error_with_cors(&req, e.status(), e.code(), &e.to_string()),
    };
    if !is_super_admin(&ctx.env, &claims) {
        return middleware::error_with_cors(&req, 403, "auth.super_admin_only", "super admin only");
    }

    // Try KV cache first (5 min TTL)
    let kv = ctx.env.kv("KV").ok();
    let cache_key = "obs:global:agg";
    if let Some(ref kv) = kv {
        if let Ok(Some(cached)) = kv.get(cache_key).text().await {
            let mut resp = Response::ok(cached)?;
            resp.headers_mut().set("content-type", "application/json")?;
            resp.headers_mut().set("x-cache", "hit")?;
            let origin = req.headers().get("Origin")?.unwrap_or_default();
            middleware::add_cors(&mut resp, Some(&origin))?;
            return Ok(resp);
        }
    }

    // Aggregate across workspaces
    let index = get_index_db(&ctx.env)?;
    let client = D1RestClient::from_env(&ctx.env)?;

    #[derive(serde::Deserialize)]
    struct WsRow {
        slug: String,
        name: Option<String>,
        plan: String,
        d1_database_id: String,
    }

    let workspaces: Vec<WsRow> = index
        .prepare("SELECT slug, name, plan, d1_database_id FROM workspaces_meta")
        .bind(&[])?
        .all()
        .await?
        .results()?;

    let mut total_cost = 0f64;
    let mut total_calls = 0i64;
    let mut by_workspace: Vec<WorkspaceStats> = vec![];
    let mut cost_by_model_map: std::collections::HashMap<(String, String), (f64, i64)> =
        std::collections::HashMap::new();
    let mut all_errors: Vec<GlobalError> = vec![];
    let mut all_signals: std::collections::HashMap<String, i64> = std::collections::HashMap::new();

    for ws in &workspaces {
        let db = WorkspaceDb::new(&client, ws.d1_database_id.clone());

        // Costs by model for this ws
        let costs = db.aggregate_llm_costs_30d().await.unwrap_or_default();
        let mut ws_cost = 0f64;
        let mut ws_calls = 0i64;
        for c in &costs {
            ws_cost += c.cost_usd;
            ws_calls += c.call_count;
            let entry = cost_by_model_map
                .entry((c.provider.clone(), c.model.clone()))
                .or_insert((0.0, 0));
            entry.0 += c.cost_usd;
            entry.1 += c.call_count;
        }
        total_cost += ws_cost;
        total_calls += ws_calls;

        // Quality signals for this ws
        let signals = db.aggregate_quality_signals_30d().await.unwrap_or_default();
        let mut praise = 0i64;
        let mut total_sig = 0i64;
        for s in &signals {
            total_sig += s.count;
            if s.signal_type == "praise" || s.signal_type == "thanks" {
                praise += s.count;
            }
            *all_signals.entry(s.signal_type.clone()).or_insert(0) += s.count;
        }
        let q_score = if total_sig > 0 {
            praise as f64 / total_sig as f64
        } else {
            1.0
        };

        by_workspace.push(WorkspaceStats {
            slug: ws.slug.clone(),
            name: ws.name.clone(),
            plan: ws.plan.clone(),
            cost_usd: ws_cost,
            calls: ws_calls,
            quality_score: q_score,
        });

        // Recent errors for this ws (top 3 each)
        let errors = db.list_recent_llm_errors(3).await.unwrap_or_default();
        for e in errors {
            all_errors.push(GlobalError {
                workspace_slug: ws.slug.clone(),
                created_at: e.created_at,
                provider: e.provider,
                model: e.model,
                error: e.error,
            });
        }
    }

    // Sort + truncate
    by_workspace.sort_by(|a, b| {
        b.cost_usd
            .partial_cmp(&a.cost_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    all_errors.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    all_errors.truncate(20);

    let mut cost_by_model: Vec<ModelCostAgg> = cost_by_model_map
        .into_iter()
        .map(|((provider, model), (cost_usd, call_count))| ModelCostAgg {
            provider,
            model,
            cost_usd,
            call_count,
        })
        .collect();
    cost_by_model.sort_by(|a, b| {
        b.cost_usd
            .partial_cmp(&a.cost_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let quality_signals: Vec<grumps_agent::db::QualitySignalCount> = all_signals
        .into_iter()
        .map(|(signal_type, count)| grumps_agent::db::QualitySignalCount { signal_type, count })
        .collect();

    let payload = GlobalObservability {
        generated_at: chrono::Utc::now().to_rfc3339(),
        workspaces_count: workspaces.len(),
        total_cost_usd: total_cost,
        total_calls,
        by_workspace,
        cost_by_model,
        recent_errors: all_errors,
        quality_signals,
    };

    let json = serde_json::to_string(&payload).map_err(|e| Error::RustError(e.to_string()))?;

    if let Some(ref kv) = kv {
        // Await the write — a dropped `execute()` future never runs, so the
        // observability cache would never actually be populated.
        if let Ok(p) = kv.put(cache_key, &json) {
            let _ = p.expiration_ttl(300).execute().await;
        }
    }

    let mut resp = Response::ok(json)?;
    resp.headers_mut().set("content-type", "application/json")?;
    resp.headers_mut().set("x-cache", "miss")?;
    let origin = req.headers().get("Origin")?.unwrap_or_default();
    middleware::add_cors(&mut resp, Some(&origin))?;
    Ok(resp)
}

/// GET /api/admin/me — returns whether the caller is super admin
/// POST /api/admin/w/:slug/scheduled/:id/fire — super-admin only.
/// Force-fires a scheduled action immediately (bypassing the DO alarm),
/// running the same `execute_action` path the alarm would. Useful as a
/// production kill-switch and for tests that can't wait on a real alarm
/// (wrangler dev's alarm subsystem doesn't fire reliably).
pub async fn force_fire_scheduled(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match middleware::verify_session(&req, &ctx.env).await {
        Ok(c) => c,
        Err(e) => return middleware::error_with_cors(&req, e.status(), e.code(), &e.to_string()),
    };
    if !is_super_admin(&ctx.env, &claims) {
        return middleware::error_with_cors(&req, 403, "auth.super_admin_only", "super admin only");
    }

    let slug = match ctx.param("slug") {
        Some(s) => s.clone(),
        None => return middleware::error_with_cors(&req, 400, "bad_request", "missing slug"),
    };
    let id = match ctx.param("id") {
        Some(s) => s.clone(),
        None => return middleware::error_with_cors(&req, 400, "bad_request", "missing id"),
    };

    let index_db = get_index_db(&ctx.env)?;
    let ws = match crate::db::lookup_workspace_by_slug(&index_db, &slug).await? {
        Some(w) => w,
        None => {
            return middleware::error_with_cors(
                &req,
                404,
                "workspace.not_found",
                "workspace not found",
            )
        }
    };

    let client = D1RestClient::from_env(&ctx.env)?;
    let db = WorkspaceDb::new(&client, ws.d1_database_id.clone());
    let action = match db.get_scheduled_action(&id).await? {
        Some(a) => a,
        None => {
            return middleware::error_with_cors(
                &req,
                404,
                "scheduled.not_found",
                "action not found",
            )
        }
    };

    if let Err(e) = crate::scheduler_executor::execute_action(&ctx.env, &slug, &action).await {
        worker::console_log!("force_fire_scheduled failed for {slug}/{id}: {e}");
        return middleware::error_with_cors(&req, 500, "execute_failed", &format!("{e}"));
    }

    let mut resp = Response::from_json(&serde_json::json!({ "ok": true, "id": id }))?;
    let origin = req.headers().get("Origin")?.unwrap_or_default();
    middleware::add_cors(&mut resp, Some(&origin))?;
    Ok(resp)
}

pub async fn whoami(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match middleware::verify_session(&req, &ctx.env).await {
        Ok(c) => c,
        Err(e) => return middleware::error_with_cors(&req, e.status(), e.code(), &e.to_string()),
    };
    let is_super = is_super_admin(&ctx.env, &claims);
    let payload = serde_json::json!({
        "is_super_admin": is_super,
        "phone": claims.phone,
    });
    let mut resp = Response::from_json(&payload)?;
    let origin = req.headers().get("Origin")?.unwrap_or_default();
    middleware::add_cors(&mut resp, Some(&origin))?;
    Ok(resp)
}

/// POST /api/admin/migrate-all — apply pending schema migrations to every
/// workspace database (super-admin only). The on-demand backfill for databases
/// provisioned before a new migration was added.
pub async fn migrate_all(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let claims = match middleware::verify_session(&req, &ctx.env).await {
        Ok(c) => c,
        Err(e) => return middleware::error_with_cors(&req, e.status(), e.code(), &e.to_string()),
    };
    if !is_super_admin(&ctx.env, &claims) {
        return middleware::error_with_cors(
            &req,
            403,
            "auth.not_super_admin",
            "super admin required",
        );
    }

    let index = get_index_db(&ctx.env)?;
    let client = D1RestClient::from_env(&ctx.env)?;

    #[derive(serde::Deserialize)]
    struct WsRow {
        slug: String,
        d1_database_id: String,
    }
    let workspaces: Vec<WsRow> = index
        .prepare("SELECT slug, d1_database_id FROM workspaces_meta")
        .bind(&[])?
        .all()
        .await?
        .results()?;

    let mut applied_total = 0usize;
    let mut results: Vec<serde_json::Value> = vec![];
    for ws in &workspaces {
        match crate::migrations::apply_pending(&client, &ws.d1_database_id).await {
            Ok(applied) => {
                applied_total += applied.len();
                results.push(serde_json::json!({ "slug": ws.slug, "applied": applied }));
            }
            Err(e) => {
                worker::console_log!("[migrate-all] {} failed: {e}", ws.slug);
                results.push(serde_json::json!({ "slug": ws.slug, "error": e.to_string() }));
            }
        }
    }

    let mut resp = Response::from_json(&serde_json::json!({
        "ok": true,
        "workspaces": workspaces.len(),
        "migrations_applied": applied_total,
        "results": results,
    }))?;
    let origin = req.headers().get("Origin")?.unwrap_or_default();
    middleware::add_cors(&mut resp, Some(&origin))?;
    Ok(resp)
}
