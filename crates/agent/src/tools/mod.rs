pub mod schemas;
pub mod memory;
pub mod rag;
pub mod crud;
pub mod scheduler;
pub mod calendar;
pub mod web;
pub mod chat;

pub async fn dispatch(
    _env: &worker::Env,
    _ws_slug: &str,
    _member_id: &str,
    _tool_name: &str,
    _args: serde_json::Value,
) -> worker::Result<serde_json::Value> {
    Err(worker::Error::RustError(
        "tool dispatch not yet implemented".into(),
    ))
}
