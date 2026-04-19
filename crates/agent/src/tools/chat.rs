//! Tool implementation: send_message.

use serde_json::Value;
use super::ToolContext;

pub async fn send_message(ctx: &ToolContext<'_>, args: Value) -> worker::Result<Value> {
    let text = args.get("text").and_then(|v| v.as_str())
        .ok_or_else(|| worker::Error::RustError("send_message: missing 'text'".into()))?;

    ctx.sink.send(text).await?;
    Ok(serde_json::json!({ "sent": true }))
}
