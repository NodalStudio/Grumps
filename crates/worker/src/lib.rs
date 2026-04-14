use worker::*;

mod billing;
mod cron;
mod d1_rest;
mod db;
mod error;
mod handler;
mod middleware;
mod provisioning;
mod llm_client;
mod routes;

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
        // Export
        .get_async("/api/w/:slug/export/todos", routes::export::export_todos)
        .get_async("/api/w/:slug/export/notes", routes::export::export_notes)
        // Stripe webhook
        .post_async("/webhook/stripe", routes::stripe_webhook::handle_stripe_webhook)
        .run(req, env)
        .await
}
