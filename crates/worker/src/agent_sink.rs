//! Adapter implementing the agent's MessagingSink trait by sending to the
//! workspace's chat platform via existing messaging_dispatch.

use worker::*;
use grumps_agent::router::{MessagingSink, ProposalButton};

pub struct WorkerMessagingSink<'a> {
    pub env: &'a Env,
    pub ws_slug: String,
}

#[async_trait::async_trait(?Send)]
impl<'a> MessagingSink for WorkerMessagingSink<'a> {
    async fn send(&self, text: &str) -> Result<()> {
        use grumps_messaging::adapter::OutboundMessage;
        let out = OutboundMessage { text: text.to_string(), ..Default::default() };
        crate::messaging_dispatch::send_to_workspace(self.env, &self.ws_slug, &out).await
    }

    async fn send_with_buttons(&self, text: &str, buttons: &[ProposalButton]) -> Result<Option<String>> {
        use grumps_messaging::adapter::OutboundMessage;
        // One button per row → a vertical stack of full-width buttons.
        let rows: Vec<serde_json::Value> = buttons
            .iter()
            .map(|b| serde_json::json!([{ "text": b.label, "callback_data": b.id }]))
            .collect();
        let out = OutboundMessage {
            text: text.to_string(),
            reply_markup: Some(serde_json::json!({ "inline_keyboard": rows })),
            ..Default::default()
        };
        crate::messaging_dispatch::send_to_workspace_with_markup(self.env, &self.ws_slug, &out).await
    }
}
