use worker::*;

mod billing;
mod cron;
mod d1_rest;
mod db;
mod durable_objects;
mod error;
mod extract;
mod handler;
mod llm_client;
mod middleware;
mod migrations;
mod provisioning;
// rag module moved to grumps_agent::tools::rag_pipeline
mod agent_db_impl;
mod agent_sink;
mod auth_telegram;
mod messaging_dispatch;
mod observability;
mod rate_limit;
mod routes;
mod scheduler_executor;

pub use durable_objects::WorkspaceScheduler;

/// Install a WASM panic hook so a panic surfaces a readable
/// `panicked at 'msg', file:line` line in `wrangler tail` / Workers Logs
/// instead of an opaque `RuntimeError: unreachable executed`.
#[event(start)]
fn start() {
    console_error_panic_hook::set_once();
}

#[event(scheduled)]
pub async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    if let Err(e) = cron::handle_cron(&env).await {
        console_log!("Cron error: {:?}", e);
    }
}

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    // CORS preflight: answered before logging to keep OPTIONS noise out.
    if req.method() == Method::Options {
        return middleware::preflight(&req);
    }

    // Correlation id (cf-ray) + request-in line, so every request — including
    // ones rejected early (403/404) — is visible and stitchable by `rid=`.
    let rid = observability::request_id(&req);
    let method = req.method().to_string();
    let path = req.path();
    let started = Date::now().as_millis();
    observability::log_request_in(&rid, &method, &path);

    let result = Router::new()
        // Health
        .get("/health", routes::health::handle)
        // WhatsApp webhook
        .get_async("/webhook/whatsapp", routes::webhook::handle_verify)
        .post_async("/webhook/whatsapp", routes::webhook::handle_incoming)
        // Telegram webhook
        .post_async(
            "/webhook/telegram",
            routes::webhook_telegram::handle_incoming,
        )
        // Discord webhook
        .post_async("/webhook/discord", routes::webhook_discord::handle_incoming)
        // Auth — legacy WA OTP (Bearer JWT)
        .post_async("/auth/otp", routes::auth::handle_send_otp)
        .post_async("/auth/verify", routes::auth::handle_verify_otp)
        // Auth — Telegram Widget + session management (cookie JWT + CSRF)
        .post_async(
            "/auth/telegram/verify",
            routes::auth::handle_telegram_verify,
        )
        .get_async("/auth/me", routes::auth::handle_me)
        .post_async("/auth/logout", routes::auth::handle_logout)
        .get_async("/auth/sessions", routes::sessions::list_sessions)
        .delete_async("/auth/sessions/:id", routes::sessions::revoke_specific)
        .post_async(
            "/auth/sessions/revoke-all",
            routes::sessions::revoke_all_others,
        )
        // Workspaces
        .get_async("/api/workspaces", routes::workspace_api::list_my_workspaces)
        .patch_async("/api/me", routes::workspace_api::update_me)
        .get_async("/api/w/:slug", routes::workspace_api::workspace_info)
        .get_async(
            "/api/w/:slug/history",
            routes::workspace_api::workspace_history,
        )
        .get_async(
            "/api/w/:slug/members",
            routes::workspace_api::workspace_members,
        )
        .patch_async(
            "/api/w/:slug/settings/locale",
            routes::workspace_api::update_locale,
        )
        .patch_async(
            "/api/w/:slug/settings/timezone",
            routes::workspace_api::update_timezone,
        )
        .patch_async(
            "/api/w/:slug/settings/name",
            routes::workspace_api::update_workspace_name,
        )
        // Todos
        .get_async("/api/w/:slug/todos", extract::route(routes::todos::list_todos))
        .post_async(
            "/api/w/:slug/todos",
            extract::route_json(routes::todos::create_todo),
        )
        .patch_async(
            "/api/w/:slug/todos/:id",
            extract::route_json(routes::todos::update_todo),
        )
        .delete_async(
            "/api/w/:slug/todos/:id",
            extract::route(routes::todos::delete_todo),
        )
        // Notes
        .get_async("/api/w/:slug/notes", routes::notes::list_notes)
        .post_async("/api/w/:slug/notes", routes::notes::create_note)
        .get_async("/api/w/:slug/notes/:id", routes::notes::get_note)
        .put_async("/api/w/:slug/notes/:id", routes::notes::update_note)
        .delete_async("/api/w/:slug/notes/:id", routes::notes::delete_note)
        // Memory
        .get_async("/api/w/:slug/memory", routes::memory::list)
        .post_async("/api/w/:slug/memory", routes::memory::create)
        .get_async("/api/w/:slug/memory/:id", routes::memory::get)
        .put_async("/api/w/:slug/memory/:id", routes::memory::update)
        .delete_async("/api/w/:slug/memory/:id", routes::memory::delete)
        // Events
        .get_async("/api/w/:slug/events", routes::events::list)
        .post_async("/api/w/:slug/events", routes::events::create)
        .get_async("/api/w/:slug/events/:id", routes::events::get)
        .put_async("/api/w/:slug/events/:id", routes::events::update)
        .delete_async("/api/w/:slug/events/:id", routes::events::delete)
        // Scheduled actions
        .get_async("/api/w/:slug/scheduled", routes::scheduled::list)
        .post_async("/api/w/:slug/scheduled", routes::scheduled::create)
        .get_async("/api/w/:slug/scheduled/:id", routes::scheduled::get)
        .delete_async("/api/w/:slug/scheduled/:id", routes::scheduled::delete)
        // Calendar aggregation + iCal
        .get_async("/api/w/:slug/calendar", routes::calendar::aggregated)
        .post_async(
            "/api/w/:slug/calendar/ical-token",
            routes::calendar::create_ical_token,
        )
        .delete_async(
            "/api/w/:slug/calendar/ical-token",
            routes::calendar::delete_ical_token,
        )
        .get_async("/cal/:slug", routes::calendar::ical_feed)
        // Export
        .get_async("/api/w/:slug/export/todos", routes::export::export_todos)
        .get_async("/api/w/:slug/export/notes", routes::export::export_notes)
        // Observability
        .get_async(
            "/api/w/:slug/admin/observability",
            routes::observability::aggregated,
        )
        // Super admin
        .get_async(
            "/api/admin/observability",
            routes::admin_global::observability,
        )
        .get_async("/api/admin/me", routes::admin_global::whoami)
        .post_async(
            "/api/admin/w/:slug/scheduled/:id/fire",
            routes::admin_global::force_fire_scheduled,
        )
        // Deploy-time migration trigger (CI, secret-gated — not super-admin).
        .post_async(
            "/internal/migrate-workspaces",
            routes::admin_global::migrate_workspaces_internal,
        )
        // Stripe webhook
        .post_async(
            "/webhook/stripe",
            routes::stripe_webhook::handle_stripe_webhook,
        )
        .run(req, env)
        .await;

    let ms = Date::now().as_millis().saturating_sub(started);
    match &result {
        Ok(resp) => observability::log_request_out(&rid, resp.status_code(), ms),
        Err(e) => observability::log_error(&rid, "fetch", &e.to_string()),
    }
    result
}
