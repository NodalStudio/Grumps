//! Tool implementation: send_message.

use serde_json::Value;
use super::{args, parse_args, ToolContext};

pub async fn send_message(ctx: &ToolContext<'_>, raw: Value) -> worker::Result<Value> {
    let a: args::SendMessageArgs = parse_args(raw, "send_message")?;
    ctx.sink.send(&a.text).await?;
    Ok(serde_json::json!({ "sent": true }))
}
