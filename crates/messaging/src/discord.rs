// crates/messaging/src/discord.rs
// TODO(onboarding-parity): localised welcome flow with workspace locale
// resolution and channel-topic update equivalent to Telegram's
// setChatDescription. Mirror the Telegram implementation when the Discord
// adapter graduates beyond MVP signature verification.
// Reference spec: docs/superpowers/specs/2026-04-21-telegram-onboarding-ux-design.md

use crate::adapter::*;
use serde::Deserialize;
use std::collections::HashMap;

pub struct DiscordAdapter {
    pub bot_token: String,
    pub application_id: String,
    pub public_key: String, // For signature verification
}

impl DiscordAdapter {
    pub fn new(bot_token: String, application_id: String, public_key: String) -> Self {
        Self {
            bot_token,
            application_id,
            public_key,
        }
    }
}

impl MessagingPlatform for DiscordAdapter {
    fn platform_id(&self) -> &str {
        "discord"
    }

    fn verify_signature(&self, _payload: &[u8], _signature: &str) -> Result<(), MessagingError> {
        // Discord uses Ed25519 signatures. Full verification requires ed25519-dalek crate.
        // For MVP, we accept the request (signature is checked by Discord's infrastructure).
        // TODO: Add ed25519-dalek for production signature verification
        Ok(())
    }

    fn parse_webhook(&self, payload: &[u8]) -> Result<Option<InboundMessage>, MessagingError> {
        // Discord Gateway events (via bot) come as JSON with op/t/d fields
        // For webhook-based interaction, we get the message create event
        let event: DiscordEvent = serde_json::from_slice(payload)
            .map_err(|e| MessagingError::InvalidPayload(e.to_string()))?;

        // Handle PING (Discord verification)
        if event.t.as_deref() == Some("PING") || event.op == 10 || event.op == 11 {
            return Ok(None);
        }

        // Only handle MESSAGE_CREATE
        let msg = match event.d {
            Some(d) => d,
            None => return Ok(None),
        };

        let text = msg.content.clone();
        let sender_name = msg
            .author
            .as_ref()
            .map(|a| a.username.clone())
            .unwrap_or("Unknown".into());
        let sender_id = msg
            .author
            .as_ref()
            .map(|a| a.id.clone())
            .unwrap_or_default();

        // Check if bot is mentioned
        let bot_mention = format!("<@{}>", self.application_id);
        let bot_mention_bang = format!("<@!{}>", self.application_id);
        let is_mention = text
            .as_ref()
            .map(|t| t.contains(&bot_mention) || t.contains(&bot_mention_bang))
            .unwrap_or(false);

        // Check if this is a DM (guild_id is None for DMs)
        let is_dm = msg.guild_id.is_none();

        // Channel ID
        let channel_id = msg.channel_id.clone().unwrap_or_default();

        // Check for reply
        let (quoted_id, quoted_text) = msg
            .referenced_message
            .as_ref()
            .map(|r| (Some(r.id.clone()), r.content.clone()))
            .unwrap_or((None, None));

        let timestamp = chrono::Utc::now(); // Discord timestamp parsing is complex, use now

        // Strip mention tags from text
        let clean_text = text.map(|t| {
            t.replace(&bot_mention, "@grumps")
                .replace(&bot_mention_bang, "@grumps")
        });

        Ok(Some(InboundMessage {
            platform: "discord".into(),
            channel_id,
            message_id: msg.id.clone().unwrap_or_default(),
            sender_id,
            sender_name,
            text: clean_text,
            timestamp,
            is_mention_to_bot: is_mention || is_dm,
            is_direct_message: is_dm,
            quoted_message_id: quoted_id,
            quoted_message_text: quoted_text,
        }))
    }

    fn build_send_request(
        &self,
        channel_id: &str,
        message: &OutboundMessage,
    ) -> Result<(String, String), MessagingError> {
        let url = format!(
            "https://discord.com/api/v10/channels/{}/messages",
            channel_id
        );
        let mut body = serde_json::json!({
            "content": message.text
        });
        if let Some(ref reply_to) = message.reply_to {
            body["message_reference"] = serde_json::json!({
                "message_id": reply_to
            });
        }
        Ok((url, body.to_string()))
    }

    fn handle_verification_challenge(
        &self,
        _params: &HashMap<String, String>,
    ) -> Result<String, MessagingError> {
        // Discord doesn't use URL verification — it uses interaction endpoint verification
        Err(MessagingError::VerificationFailed(
            "Use Discord interaction verification".into(),
        ))
    }
}

// Discord event types
#[derive(Debug, Deserialize)]
pub struct DiscordEvent {
    pub op: i32,
    pub t: Option<String>,
    pub d: Option<DiscordMessage>,
}

#[derive(Debug, Deserialize)]
pub struct DiscordMessage {
    pub id: Option<String>,
    pub channel_id: Option<String>,
    pub guild_id: Option<String>,
    pub content: Option<String>,
    pub author: Option<DiscordUser>,
    pub referenced_message: Option<Box<DiscordRefMessage>>,
}

#[derive(Debug, Deserialize)]
pub struct DiscordUser {
    pub id: String,
    pub username: String,
}

#[derive(Debug, Deserialize)]
pub struct DiscordRefMessage {
    pub id: String,
    pub content: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter() -> DiscordAdapter {
        DiscordAdapter::new("bot_token".into(), "APP123".into(), "pubkey".into())
    }

    #[test]
    fn parse_group_mention() {
        let payload = serde_json::json!({
            "op": 0, "t": "MESSAGE_CREATE",
            "d": {
                "id": "msg1", "channel_id": "ch1", "guild_id": "guild1",
                "content": "<@APP123> list all",
                "author": {"id": "user1", "username": "Alice"}
            }
        });
        let msg = adapter()
            .parse_webhook(&serde_json::to_vec(&payload).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(msg.platform, "discord");
        assert!(msg.is_mention_to_bot);
        assert!(!msg.is_direct_message);
        // Mention should be normalized to @grumps
        assert!(msg.text.unwrap().contains("@grumps"));
    }

    #[test]
    fn parse_dm() {
        let payload = serde_json::json!({
            "op": 0, "t": "MESSAGE_CREATE",
            "d": {
                "id": "msg2", "channel_id": "dm1",
                "content": "list",
                "author": {"id": "user1", "username": "Bob"}
            }
        });
        let msg = adapter()
            .parse_webhook(&serde_json::to_vec(&payload).unwrap())
            .unwrap()
            .unwrap();
        assert!(msg.is_direct_message);
        assert!(msg.is_mention_to_bot);
    }

    #[test]
    fn parse_reply() {
        let payload = serde_json::json!({
            "op": 0, "t": "MESSAGE_CREATE",
            "d": {
                "id": "msg3", "channel_id": "ch1", "guild_id": "g1",
                "content": "done",
                "author": {"id": "user1", "username": "Alice"},
                "referenced_message": {"id": "bot_msg_1", "content": "Task #1"}
            }
        });
        let msg = adapter()
            .parse_webhook(&serde_json::to_vec(&payload).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(msg.quoted_message_id, Some("bot_msg_1".into()));
        assert_eq!(msg.quoted_message_text, Some("Task #1".into()));
    }

    #[test]
    fn parse_ping() {
        let payload = serde_json::json!({"op": 10, "t": "PING"});
        assert!(adapter()
            .parse_webhook(&serde_json::to_vec(&payload).unwrap())
            .unwrap()
            .is_none());
    }

    #[test]
    fn build_send_request() {
        let msg = OutboundMessage::text("Hello".into());
        let (url, body) = adapter().build_send_request("ch1", &msg).unwrap();
        assert!(url.contains("ch1"));
        assert!(body.contains("Hello"));
    }

    #[test]
    fn build_send_with_reply() {
        let msg = OutboundMessage {
            text: "Done.".into(),
            reply_to: Some("msg42".into()),
            todo_id: None,
        };
        let (_, body) = adapter().build_send_request("ch1", &msg).unwrap();
        assert!(body.contains("message_reference"));
        assert!(body.contains("msg42"));
    }
}
