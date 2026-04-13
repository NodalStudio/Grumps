use worker::*;
mod d1_rest;
mod db;
mod error;
mod handler;
mod provisioning;
mod routes;

#[event(fetch, respond_with_errors)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    Router::new()
        .get("/health", |_, _| Response::ok("ok"))
        .run(req, env)
        .await
}
