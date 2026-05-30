//! Observability endpoint — aggregated LLM telemetry for admin dashboard.
//! GET /api/w/:slug/admin/observability

use worker::*;
use serde::Serialize;
use crate::{db, middleware, d1_rest};

// ── helpers ───────────────────────────────────────────────────────────────────

async fn resolve_workspace(ctx: &RouteContext<()>) -> Result<db::WorkspaceMetaRow> {
    let slug = ctx.param("slug").ok_or_else(|| Error::RustError("missing slug".into()))?;
    let index_db = db::get_index_db(&ctx.env)?;
    db::lookup_workspace_by_slug(&index_db, slug).await?
        .ok_or_else(|| Error::RustError("workspace not found".into()))
}

// ── Response shape ────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct CascadeEfficiency {
    pub classifier_resolved: i64,
    pub sonnet_escalated: i64,
    /// Estimated savings (Gemini flash vs hypothetical Sonnet for the same calls)
    pub saved_usd: f64,
}

#[derive(Serialize)]
pub struct ObservabilityResponse {
    pub month: String,
    pub total_cost_usd: f64,
    pub total_calls: i64,
    pub median_latency_ms: i64,
    /// praise+thanks / (praise+thanks+silence+forget+correction+confusion)
    pub quality_score: f64,
    pub cost_by_model: Vec<grumps_agent::db::LlmCostByModel>,
    pub latency_by_model: Vec<grumps_agent::db::LlmLatencyByModel>,
    pub invocation_types: Vec<grumps_agent::db::LlmInvocationCount>,
    pub cascade_efficiency: CascadeEfficiency,
    pub top_tools: Vec<serde_json::Value>,
    pub recent_errors: Vec<grumps_agent::db::LlmErrorEntry>,
    pub quality_signals: Vec<grumps_agent::db::QualitySignalCount>,
}

// ── GET /api/w/:slug/admin/observability ─────────────────────────────────────

pub async fn aggregated(req: Request, ctx: RouteContext<()>) -> Result<Response> {
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

    // Super admin only
    if !middleware::is_super_admin(&ctx.env, &claims) {
        return middleware::error_with_cors(&req, 403, "auth.super_admin_only", "super admin only");
    }

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, ws.d1_database_id.clone());

    // Run all aggregation queries concurrently (not true parallelism on WASM but at least sequential)
    let cost_by_model = ws_db.aggregate_llm_costs_30d().await.unwrap_or_default();
    let latency_by_model = ws_db.aggregate_llm_latency_by_model().await.unwrap_or_default();
    let invocation_types = ws_db.aggregate_llm_invocation_types().await.unwrap_or_default();
    let recent_errors = ws_db.list_recent_llm_errors(20).await.unwrap_or_default();
    let quality_signals = ws_db.aggregate_quality_signals_30d().await.unwrap_or_default();

    // Derived totals
    let total_cost_usd: f64 = cost_by_model.iter().map(|c| c.cost_usd).sum();
    let total_calls: i64 = cost_by_model.iter().map(|c| c.call_count).sum();

    // Median latency — use p50 from the first model entry, or derive globally
    let median_latency_ms = latency_by_model.first().map(|m| m.p50_ms).unwrap_or(0);

    // Quality score: (praise + thanks) / total signals
    let count_signal = |name: &str| -> i64 {
        quality_signals.iter().find(|s| s.signal_type == name).map(|s| s.count).unwrap_or(0)
    };
    let positive = count_signal("praise") + count_signal("thanks");
    let total_signals: i64 = quality_signals.iter().map(|s| s.count).sum();
    let quality_score = if total_signals > 0 { positive as f64 / total_signals as f64 } else { 1.0 };

    // Cascade efficiency
    let classifier_resolved = invocation_types.iter().find(|i| i.invocation_type == "classify").map(|i| i.count).unwrap_or(0);
    let sonnet_escalated = invocation_types.iter()
        .filter(|i| i.invocation_type == "agent_react" || i.invocation_type == "agent_oneshot")
        .map(|i| i.count).sum::<i64>();
    // Savings estimate: each classify call saved ~(3.0 input + 15.0 output) * avg_tokens vs gemini (0.15+0.60)
    // Use a rough per-call delta of $0.002 (typical 500-token interaction on Sonnet vs Gemini Flash)
    let saved_usd = (classifier_resolved as f64) * 0.002;

    // Top tools: derive from tool_calls JSON in llm_calls (expensive — skip for now, return empty)
    // TODO: implement by querying llm_calls.tool_calls JSON array aggregate
    let top_tools: Vec<serde_json::Value> = vec![];

    // Current month label
    let month = chrono::Utc::now().format("%Y-%m").to_string();

    let body = ObservabilityResponse {
        month,
        total_cost_usd,
        total_calls,
        median_latency_ms,
        quality_score,
        cost_by_model,
        latency_by_model,
        invocation_types,
        cascade_efficiency: CascadeEfficiency {
            classifier_resolved,
            sonnet_escalated,
            saved_usd,
        },
        top_tools,
        recent_errors,
        quality_signals,
    };

    let json = serde_json::to_string(&body)
        .map_err(|e| Error::RustError(format!("serialize observability: {e}")))?;
    let mut resp = Response::ok(json)?;
    resp.headers_mut().set("Content-Type", "application/json")?;
    // CORS
    resp.headers_mut().set("Access-Control-Allow-Origin", "*")?;
    Ok(resp)
}
