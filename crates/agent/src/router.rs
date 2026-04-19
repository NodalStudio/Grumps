use serde::Serialize;

pub struct RouteResult {
    pub final_text: Option<String>,
}

#[async_trait::async_trait]
pub trait MessagingSink: Send + Sync {
    async fn send(&self, text: &str) -> worker::Result<()>;
}

pub async fn route_message(
    _env: &worker::Env,
    _ws_slug: &str,
    _member_id: &str,
    _text: &str,
    _has_active_session: bool,
    _sink: &(dyn MessagingSink + Send + Sync),
) -> worker::Result<RouteResult> {
    Ok(RouteResult { final_text: None })
}
