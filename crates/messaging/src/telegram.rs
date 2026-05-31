use crate::adapter::*;
use serde::Deserialize;
use std::collections::HashMap;

pub struct TelegramAdapter {
    pub bot_token: String,
    pub bot_username: String,  // without @
    pub webhook_secret: String,
}

impl TelegramAdapter {
    pub fn new(bot_token: String, bot_username: String, webhook_secret: String) -> Self {
        Self { bot_token, bot_username, webhook_secret }
    }

    /// The Bot API endpoint URL for a given method (single source of the base URL
    /// + token interpolation).
    fn api_url(&self, method: &str) -> String {
        format!("https://api.telegram.org/bot{}/{method}", self.bot_token)
    }

    /// Build request to set group description via Telegram Bot API.
    pub fn build_set_description_request(&self, chat_id: &str, description: &str) -> Result<(String, String), MessagingError> {
        let url = self.api_url("setChatDescription");
        let body = serde_json::json!({
            "chat_id": chat_id,
            "description": description
        });
        Ok((url, body.to_string()))
    }

    /// Build an `answerCallbackQuery` request — acknowledges an inline-button tap
    /// (dismisses the client's loading spinner). An optional `text` shows a toast.
    pub fn build_answer_callback_request(&self, callback_query_id: &str, text: Option<&str>) -> (String, String) {
        let url = self.api_url("answerCallbackQuery");
        let mut body = serde_json::json!({ "callback_query_id": callback_query_id });
        if let Some(t) = text {
            body["text"] = serde_json::json!(t);
        }
        (url, body.to_string())
    }

    /// Build an `editMessageReplyMarkup` request — swaps (or, with `markup: None`,
    /// removes) the inline keyboard on an already-sent message.
    pub fn build_edit_reply_markup_request(&self, chat_id: &str, message_id: i64, markup: Option<serde_json::Value>) -> (String, String) {
        let url = self.api_url("editMessageReplyMarkup");
        let mut body = serde_json::json!({ "chat_id": chat_id, "message_id": message_id });
        // Omitting reply_markup removes the keyboard.
        if let Some(m) = markup {
            body["reply_markup"] = m;
        }
        (url, body.to_string())
    }
}

impl MessagingPlatform for TelegramAdapter {
    fn platform_id(&self) -> &str { "telegram" }

    fn verify_signature(&self, _payload: &[u8], secret: &str) -> Result<(), MessagingError> {
        // Telegram uses X-Telegram-Bot-Api-Secret-Token header
        if secret == self.webhook_secret {
            Ok(())
        } else {
            Err(MessagingError::InvalidSignature)
        }
    }

    fn parse_webhook(&self, payload: &[u8]) -> Result<Option<InboundMessage>, MessagingError> {
        let update: TgUpdate = serde_json::from_slice(payload)
            .map_err(|e| MessagingError::InvalidPayload(e.to_string()))?;

        let msg = match update.message {
            Some(m) => m,
            None => return Ok(None), // Could be an edit, callback, etc.
        };

        let text = msg.text.clone();
        let sender_name = msg.from.as_ref()
            .map(|u| {
                let mut name = u.first_name.clone();
                if let Some(ref last) = u.last_name {
                    name.push(' ');
                    name.push_str(last);
                }
                name
            })
            .unwrap_or_else(|| "Unknown".into());

        let sender_id = msg.from.as_ref()
            .map(|u| u.id.to_string())
            .unwrap_or_default();

        let is_private = msg.chat.chat_type == "private";

        // Detect @bot mention
        let bot_mention = format!("@{}", self.bot_username);
        let is_mention = text.as_ref()
            .map(|t| t.to_lowercase().contains(&bot_mention.to_lowercase()))
            .unwrap_or(false);

        // Check if reply to bot
        let is_reply_to_bot = msg.reply_to_message.as_ref()
            .and_then(|r| r.from.as_ref())
            .map(|u| u.is_bot.unwrap_or(false))
            .unwrap_or(false);

        let (quoted_id, quoted_text) = msg.reply_to_message.as_ref()
            .map(|r| (
                Some(r.message_id.to_string()),
                r.text.clone(),
            ))
            .unwrap_or((None, None));

        let timestamp = chrono::DateTime::from_timestamp(msg.date, 0)
            .unwrap_or_else(chrono::Utc::now);

        Ok(Some(InboundMessage {
            platform: "telegram".into(),
            channel_id: msg.chat.id.to_string(),
            message_id: msg.message_id.to_string(),
            sender_id,
            sender_name,
            text,
            timestamp,
            is_mention_to_bot: is_mention || is_private || is_reply_to_bot,
            is_direct_message: is_private,
            quoted_message_id: quoted_id,
            quoted_message_text: quoted_text,
        }))
    }

    fn build_send_request(&self, chat_id: &str, message: &OutboundMessage) -> Result<(String, String), MessagingError> {
        let url = self.api_url("sendMessage");
        let mut body = serde_json::json!({
            "chat_id": chat_id,
            "text": message.text,
            "parse_mode": "Markdown"
        });
        if let Some(ref reply_to) = message.reply_to {
            if let Ok(id) = reply_to.parse::<i64>() {
                body["reply_to_message_id"] = serde_json::json!(id);
            }
        }
        if let Some(ref markup) = message.reply_markup {
            body["reply_markup"] = markup.clone();
        }
        Ok((url, body.to_string()))
    }

    fn handle_verification_challenge(&self, _params: &HashMap<String, String>) -> Result<String, MessagingError> {
        // Telegram doesn't have a verification challenge like WhatsApp
        // The webhook is set via the Bot API setWebhook method
        Err(MessagingError::VerificationFailed("Telegram doesn't use verification challenges".into()))
    }
}

// Telegram Update types
#[derive(Debug, Deserialize)]
pub struct TgUpdate {
    pub update_id: i64,
    pub message: Option<TgMessage>,
    pub my_chat_member: Option<TgChatMemberUpdated>,
    /// Present when a user taps an inline keyboard button.
    pub callback_query: Option<TgCallbackQuery>,
}

/// An inline-button tap. `data` is the opaque `callback_data` we set on the
/// button; `message` is the bot message the keyboard was attached to.
#[derive(Debug, Deserialize)]
pub struct TgCallbackQuery {
    pub id: String,
    pub from: TgUser,
    pub message: Option<TgCallbackMessage>,
    pub data: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TgCallbackMessage {
    pub message_id: i64,
    pub chat: TgChat,
}

#[derive(Debug, Deserialize)]
pub struct TgChatMemberUpdated {
    pub chat: TgChat,
    pub from: TgUser,
    pub old_chat_member: TgChatMember,
    pub new_chat_member: TgChatMember,
}

#[derive(Debug, Deserialize)]
pub struct TgChatMember {
    pub status: String, // "member", "administrator", "left", "kicked"
    pub user: TgUser,
}

#[derive(Debug, Deserialize)]
pub struct TgMessage {
    pub message_id: i64,
    pub from: Option<TgUser>,
    pub chat: TgChat,
    pub date: i64,
    pub text: Option<String>,
    pub reply_to_message: Option<Box<TgReplyMessage>>,
}

#[derive(Debug, Deserialize)]
pub struct TgUser {
    pub id: i64,
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub is_bot: Option<bool>,
    pub language_code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TgChat {
    pub id: i64,
    #[serde(rename = "type")]
    pub chat_type: String,
    pub title: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TgReplyMessage {
    pub message_id: i64,
    pub from: Option<TgUser>,
    pub text: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter() -> TelegramAdapter {
        TelegramAdapter::new("123:ABC".into(), "grumps_bot".into(), "secret123".into())
    }

    #[test]
    fn verify_signature_ok() {
        assert!(adapter().verify_signature(b"", "secret123").is_ok());
    }

    #[test]
    fn verify_signature_fail() {
        assert!(adapter().verify_signature(b"", "wrong").is_err());
    }

    #[test]
    fn parse_group_message() {
        let payload = serde_json::json!({
            "update_id": 1,
            "message": {
                "message_id": 42,
                "from": {"id": 123, "first_name": "Alice", "last_name": "Martin", "is_bot": false},
                "chat": {"id": -100123, "type": "group", "title": "Roommates"},
                "date": 1713000000,
                "text": "@grumps_bot list"
            }
        });
        let msg = adapter().parse_webhook(&serde_json::to_vec(&payload).unwrap()).unwrap().unwrap();
        assert_eq!(msg.platform, "telegram");
        assert_eq!(msg.sender_name, "Alice Martin");
        assert_eq!(msg.channel_id, "-100123");
        assert!(msg.is_mention_to_bot);
        assert!(!msg.is_direct_message);
    }

    #[test]
    fn parse_private_message() {
        let payload = serde_json::json!({
            "update_id": 2,
            "message": {
                "message_id": 43,
                "from": {"id": 123, "first_name": "Bob", "is_bot": false},
                "chat": {"id": 123, "type": "private"},
                "date": 1713000000,
                "text": "list"
            }
        });
        let msg = adapter().parse_webhook(&serde_json::to_vec(&payload).unwrap()).unwrap().unwrap();
        assert!(msg.is_direct_message);
        assert!(msg.is_mention_to_bot); // DM always counts as mention
    }

    #[test]
    fn parse_reply_to_bot() {
        let payload = serde_json::json!({
            "update_id": 3,
            "message": {
                "message_id": 44,
                "from": {"id": 123, "first_name": "Alice", "is_bot": false},
                "chat": {"id": -100123, "type": "group"},
                "date": 1713000000,
                "text": "done",
                "reply_to_message": {
                    "message_id": 40,
                    "from": {"id": 999, "first_name": "Grumps", "is_bot": true},
                    "text": "Task #1..."
                }
            }
        });
        let msg = adapter().parse_webhook(&serde_json::to_vec(&payload).unwrap()).unwrap().unwrap();
        assert!(msg.is_mention_to_bot); // reply to bot counts
        assert_eq!(msg.quoted_message_id, Some("40".into()));
    }

    #[test]
    fn parse_no_message() {
        let payload = serde_json::json!({"update_id": 4});
        assert!(adapter().parse_webhook(&serde_json::to_vec(&payload).unwrap()).unwrap().is_none());
    }

    #[test]
    fn build_send_request_group() {
        let msg = OutboundMessage { text: "Hello".into(), reply_to: None, reply_markup: None };
        let (url, body) = adapter().build_send_request("-100123", &msg).unwrap();
        assert!(url.contains("123:ABC"));
        assert!(body.contains("-100123"));
        assert!(body.contains("Hello"));
    }

    #[test]
    fn parse_bot_added_event() {
        let payload = serde_json::json!({
            "update_id": 10,
            "my_chat_member": {
                "chat": {"id": -100999, "type": "group", "title": "Test Group"},
                "from": {"id": 123, "first_name": "Alice", "is_bot": false},
                "old_chat_member": {
                    "status": "left",
                    "user": {"id": 888, "first_name": "Grumps", "is_bot": true}
                },
                "new_chat_member": {
                    "status": "member",
                    "user": {"id": 888, "first_name": "Grumps", "is_bot": true}
                }
            }
        });
        let update: TgUpdate = serde_json::from_value(payload).unwrap();
        assert!(update.my_chat_member.is_some());
        assert!(update.message.is_none());
        let member = update.my_chat_member.unwrap();
        assert_eq!(member.new_chat_member.status, "member");
        assert_eq!(member.old_chat_member.status, "left");
        assert_eq!(member.chat.id, -100999);
        assert_eq!(member.chat.title, Some("Test Group".into()));
    }

    #[test]
    fn build_set_description() {
        let (url, body) = adapter().build_set_description_request("-100999", "Grumps workspace").unwrap();
        assert!(url.contains("setChatDescription"));
        assert!(body.contains("-100999"));
        assert!(body.contains("Grumps workspace"));
    }

    #[test]
    fn build_send_request_with_reply() {
        let msg = OutboundMessage { text: "Done.".into(), reply_to: Some("42".into()), reply_markup: None };
        let (_, body) = adapter().build_send_request("-100123", &msg).unwrap();
        assert!(body.contains("reply_to_message_id"));
        assert!(body.contains("42"));
    }

    #[test]
    fn build_send_request_with_markup() {
        let msg = OutboundMessage {
            text: "Mark #4 done?".into(),
            reply_to: None,
            reply_markup: Some(serde_json::json!({
                "inline_keyboard": [[{ "text": "✓ Yes", "callback_data": "pa:confirm" }]]
            })),
        };
        let (_, body) = adapter().build_send_request("-100123", &msg).unwrap();
        assert!(body.contains("inline_keyboard"));
        assert!(body.contains("pa:confirm"));
    }

    #[test]
    fn build_answer_and_edit_markup_requests() {
        let (url, body) = adapter().build_answer_callback_request("cbq123", Some("Done"));
        assert!(url.contains("answerCallbackQuery"));
        assert!(body.contains("cbq123"));
        assert!(body.contains("Done"));

        let undo = serde_json::json!({ "inline_keyboard": [[{ "text": "↩️ Undo", "callback_data": "pa:undo" }]] });
        let (url, body) = adapter().build_edit_reply_markup_request("-100123", 77, Some(undo));
        assert!(url.contains("editMessageReplyMarkup"));
        assert!(body.contains("\"message_id\":77"));
        assert!(body.contains("pa:undo"));

        // markup: None removes the keyboard (no reply_markup field emitted).
        let (_, body) = adapter().build_edit_reply_markup_request("-100123", 77, None);
        assert!(!body.contains("reply_markup"));
    }

    #[test]
    fn parse_callback_query() {
        let payload = serde_json::json!({
            "update_id": 12,
            "callback_query": {
                "id": "cbq999",
                "from": {"id": 123, "first_name": "Alice", "is_bot": false},
                "message": {
                    "message_id": 55,
                    "chat": {"id": -100123, "type": "group"}
                },
                "data": "pa:confirm"
            }
        });
        let update: TgUpdate = serde_json::from_value(payload).unwrap();
        let cbq = update.callback_query.expect("callback_query parsed");
        assert_eq!(cbq.id, "cbq999");
        assert_eq!(cbq.data.as_deref(), Some("pa:confirm"));
        let m = cbq.message.expect("message present");
        assert_eq!(m.message_id, 55);
        assert_eq!(m.chat.id, -100123);
    }

    #[test]
    fn parse_user_with_language_code() {
        let payload = serde_json::json!({
            "update_id": 1,
            "message": {
                "message_id": 42,
                "from": {"id": 123, "first_name": "Alice", "is_bot": false, "language_code": "fr"},
                "chat": {"id": -100123, "type": "group", "title": "Roommates"},
                "date": 1713000000,
                "text": "hello"
            }
        });
        let update: TgUpdate = serde_json::from_value(payload).unwrap();
        let user = update.message.unwrap().from.unwrap();
        assert_eq!(user.language_code.as_deref(), Some("fr"));
    }

    #[test]
    fn parse_my_chat_member_with_transition() {
        let payload = serde_json::json!({
            "update_id": 11,
            "my_chat_member": {
                "chat": {"id": -100999, "type": "group", "title": "Test"},
                "from": {"id": 123, "first_name": "Alice", "is_bot": false, "language_code": "de"},
                "old_chat_member": {
                    "status": "member",
                    "user": {"id": 888, "first_name": "Grumps", "is_bot": true}
                },
                "new_chat_member": {
                    "status": "administrator",
                    "user": {"id": 888, "first_name": "Grumps", "is_bot": true}
                }
            }
        });
        let update: TgUpdate = serde_json::from_value(payload).unwrap();
        let mcm = update.my_chat_member.unwrap();
        assert_eq!(mcm.old_chat_member.status, "member");
        assert_eq!(mcm.new_chat_member.status, "administrator");
        assert_eq!(mcm.from.language_code.as_deref(), Some("de"));
    }
}
