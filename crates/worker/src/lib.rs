use worker::*;

mod billing;
mod cron;
mod d1_rest;
mod db;
mod durable_objects;
mod error;
mod handler;
mod middleware;
mod provisioning;
mod llm_client;
// rag module moved to grumps_agent::tools::rag_pipeline
mod agent_db_impl;
mod agent_sink;
mod messaging_dispatch;
mod routes;
mod scheduler_executor;

pub use durable_objects::WorkspaceScheduler;

#[event(scheduled)]
pub async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    if let Err(e) = cron::handle_cron(&env).await {
        console_log!("Cron error: {:?}", e);
    }
}

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    // Handle CORS preflight for all paths
    if req.method() == Method::Options {
        return middleware::preflight(&req);
    }

    Router::new()
        // Health
        .get("/health", routes::health::handle)
        // WhatsApp webhook
        .get_async("/webhook/whatsapp", routes::webhook::handle_verify)
        .post_async("/webhook/whatsapp", routes::webhook::handle_incoming)
        // Telegram webhook
        .post_async("/webhook/telegram", routes::webhook_telegram::handle_incoming)
        // Discord webhook
        .post_async("/webhook/discord", routes::webhook_discord::handle_incoming)
        // Auth
        .post_async("/auth/otp", routes::auth::handle_send_otp)
        .post_async("/auth/verify", routes::auth::handle_verify_otp)
        // Workspaces
        .get_async("/api/workspaces", routes::workspace_api::list_my_workspaces)
        .get_async("/api/w/:slug", routes::workspace_api::workspace_info)
        .get_async("/api/w/:slug/history", routes::workspace_api::workspace_history)
        .get_async("/api/w/:slug/members", routes::workspace_api::workspace_members)
        // Todos
        .get_async("/api/w/:slug/todos", routes::todos::list_todos)
        .post_async("/api/w/:slug/todos", routes::todos::create_todo)
        .patch_async("/api/w/:slug/todos/:id", routes::todos::update_todo)
        .delete_async("/api/w/:slug/todos/:id", routes::todos::delete_todo)
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
        .post_async("/api/w/:slug/calendar/ical-token", routes::calendar::create_ical_token)
        .delete_async("/api/w/:slug/calendar/ical-token", routes::calendar::delete_ical_token)
        .get_async("/cal/:slug", routes::calendar::ical_feed)
        // Export
        .get_async("/api/w/:slug/export/todos", routes::export::export_todos)
        .get_async("/api/w/:slug/export/notes", routes::export::export_notes)
        // Stripe webhook
        .post_async("/webhook/stripe", routes::stripe_webhook::handle_stripe_webhook)
        .run(req, env)
        .await
}
