//! Tool implementation: send_message.

use super::{args, parse_args, ToolContext};
use serde_json::Value;

pub async fn send_message(ctx: &ToolContext<'_>, raw: Value) -> worker::Result<Value> {
    let a: args::SendMessageArgs = parse_args(raw, "send_message")?;
    ctx.sink.send(&a.text).await?;
    Ok(serde_json::json!({ "sent": true }))
}
