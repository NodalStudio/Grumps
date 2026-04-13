use worker::*;

mod d1_rest;
mod db;
mod error;
mod handler;
mod middleware;
mod provisioning;
mod routes;

#[event(fetch, respond_with_errors)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    // Handle CORS preflight for all paths
    if req.method() == Method::Options {
        return middleware::preflight(&req);
    }

    Router::new()
        .get("/health", routes::health::handle)
        .get_async("/webhook/whatsapp", routes::webhook::handle_verify)
        .post_async("/webhook/whatsapp", routes::webhook::handle_incoming)
        .run(req, env)
        .await
}
