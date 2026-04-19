pub struct LoopResult {
    pub final_text: Option<String>,
}

pub async fn run_loop(
    _env: &worker::Env,
    _ws_slug: &str,
    _member_id: &str,
    _user_message: &str,
    _sink: &(dyn crate::router::MessagingSink + Send + Sync),
) -> worker::Result<LoopResult> {
    Ok(LoopResult { final_text: None })
}

pub async fn run_oneshot(
    _env: &worker::Env,
    _ws_slug: &str,
    _instruction: &str,
) -> worker::Result<LoopResult> {
    Ok(LoopResult { final_text: None })
}
