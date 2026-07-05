//! Adapter implementing the agent's MessagingSink trait by sending to the
//! workspace's chat platform via existing messaging_dispatch.

use grumps_agent::router::MessagingSink;
use worker::*;

pub struct WorkerMessagingSink<'a> {
    pub env: &'a Env,
    pub ws_slug: String,
}

#[async_trait::async_trait(?Send)]
impl<'a> MessagingSink for WorkerMessagingSink<'a> {
    async fn send(&self, text: &str) -> Result<()> {
        use grumps_messaging::adapter::OutboundMessage;
        let out = OutboundMessage::text(text.to_string());
        // Intentionally not tracked as a bot message: these are the agent's
        // conversational replies, not task cards. Recording them would make a
        // "done" reply resolve to no todo yet report success. Conversation
        // continuity is handled by the agent session, not the card-reply path.
        crate::messaging_dispatch::send_to_workspace(self.env, &self.ws_slug, &out).await?;
        Ok(())
    }
}
