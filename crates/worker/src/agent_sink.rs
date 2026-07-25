//! Adapter implementing the agent's MessagingSink trait by sending to the
//! workspace's chat platform via existing messaging_dispatch.

use crate::db::WorkspaceDb;
use grumps_agent::router::MessagingSink;
use worker::*;

pub struct WorkerMessagingSink<'a, 'b> {
    pub env: &'a Env,
    pub ws_slug: String,
    pub ws_db: &'a WorkspaceDb<'b>,
}

#[async_trait::async_trait(?Send)]
impl<'a, 'b> MessagingSink for WorkerMessagingSink<'a, 'b> {
    async fn send(&self, text: &str) -> Result<()> {
        use grumps_messaging::adapter::OutboundMessage;
        let out = OutboundMessage::text(text.to_string());
        let sent_id =
            crate::messaging_dispatch::send_to_workspace(self.env, &self.ws_slug, &out).await?;
        // Tracked (todo_id: None) so a reply to this message is recognized as
        // a reply to Grumps (is_reply_to_bot) and the agent stays engaged in
        // the thread. It's not linked to a todo, so handle_card_reply's
        // no_target guard prevents a stray "done"/"delete" reply to it from
        // ever reporting a false success.
        if let Some(sent_id) = sent_id {
            let _ = self.ws_db.track_bot_message(&sent_id, None).await;
        }
        Ok(())
    }
}
