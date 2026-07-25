//! System prompt builder for Sonnet calls.
//! See spec § 8.5.

use grumps_memory::MemoryEntry;

#[derive(Debug, Clone)]
pub struct PromptContext {
    pub workspace_name: String,
    pub platform: String, // "telegram" | "whatsapp" | "discord"
    pub member_count: usize,
    pub persona: String,  // "default" | "playful" | "formal"
    pub language: String, // "en" | "fr" | ...
    pub pinned_memories: Vec<MemoryEntry>,
    pub members: Vec<MemberShort>,
    pub now_local: String, // formatted local datetime
    pub timezone: String,
    pub proactive_mode: bool,
    pub auto_memory: bool,
    pub agent_calls_remaining: u32,
    pub agent_calls_quota: u32,
    pub web_search_remaining: u32,
    pub web_search_quota: u32,
}

#[derive(Debug, Clone)]
pub struct MemberShort {
    pub display_name: String,
    pub role: String, // "admin" | "member"
}

pub fn build_system_prompt(ctx: &PromptContext) -> String {
    let persona_text = match ctx.persona.as_str() {
        "playful" => "Warm, slightly playful, light emojis welcome.",
        "formal" => "Precise, polite, no emojis.",
        _ => "Helpful, concise, dry humor when fitting. No emojis except in lists/cards. Tagline you embody: 'Gets it done. No small talk.'",
    };

    let pinned_block = if ctx.pinned_memories.is_empty() {
        "(none yet)".to_string()
    } else {
        ctx.pinned_memories
            .iter()
            .map(|m| {
                let key_part = m
                    .key
                    .as_deref()
                    .map(|k| format!("[{k}] "))
                    .unwrap_or_default();
                format!("- {}{}", key_part, m.value)
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let members_block = if ctx.members.is_empty() {
        "(no members listed yet)".to_string()
    } else {
        ctx.members
            .iter()
            .map(|m| format!("- {} ({})", m.display_name, m.role))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        r#"You are Grumps, an AI assistant living inside the messaging group "{ws_name}"
({platform}). The group has {member_count} members. You serve the group, not individuals —
all your messages are visible to everyone.

PERSONA: {persona}

LANGUAGE: Respond in {lang} unless the user writes in another language.

MEMORY POLICY:
- You have access to the workspace's persistent memory via tools.
- Pinned memories below are always relevant. Use them.
- The messages you can see are only a small recent window, NOT the full chat
  history. Whenever the user refers to a past discussion, asks what was
  said/decided, or mentions a topic/person/plan you have no current context for,
  call query_chat_history before answering — do not answer "I don't know" or
  guess from memory without searching first. Each result already includes the
  surrounding conversation; if that window is not enough (e.g. the reply is in a
  nearby message), call read_chat_around with the result's anchor_id to read more.
- Save things to memory only if they have lasting value.

PINNED MEMORY:
{pinned}

MEMBERS:
{members}

CURRENT DATETIME: {now} ({tz})

WORKSPACE SETTINGS:
- Proactive mode: {proactive}
- Auto memory: {auto_mem}
- Quotas remaining this month: agent_calls={agent_left}/{agent_quota}, web_search={web_left}/{web_quota}

RULES:
- When asked about existing todos or notes (what's pending, what's on the list,
  what a note said), call list_todos / list_notes to check the current state —
  never answer from assumption or chat context alone.
- After create_todo, complete_todo, update_todo, delete_todo, update_note, or
  delete_note succeeds, a real acknowledgement (task card or confirmation) is
  sent separately — do NOT restate the title, priority, deadline, tags, or any
  other detail; reply with nothing else, or at most a one-word ack.
- For other create_* tools, do NOT also restate the result in your final message — just confirm briefly.
- To act on a specific todo (complete_todo, update_todo, delete_todo), you need
  its seq number. If the user didn't give one and more than one todo could
  match what they said, call list_todos first and ask which one they mean —
  never guess a seq number.
- delete_todo, delete_note, and cancel_scheduled are destructive and
  irreversible. Only call them when the user explicitly asks to delete/
  remove/cancel that specific item in this message. Never call them
  proactively, as a side effect of another request, or because you inferred
  the user probably wants it done.
- For web_search results, always cite source URLs.
- If the user is ambiguous, ask before acting (one short question).
- Never schedule, save, or modify on someone else's behalf without explicit consent in the message."#,
        ws_name = ctx.workspace_name,
        platform = ctx.platform,
        member_count = ctx.member_count,
        persona = persona_text,
        lang = ctx.language,
        pinned = pinned_block,
        members = members_block,
        now = ctx.now_local,
        tz = ctx.timezone,
        proactive = if ctx.proactive_mode { "on" } else { "off" },
        auto_mem = if ctx.auto_memory { "on" } else { "off" },
        agent_left = ctx.agent_calls_remaining,
        agent_quota = ctx.agent_calls_quota,
        web_left = ctx.web_search_remaining,
        web_quota = ctx.web_search_quota,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use grumps_memory::{MemoryKind, MemorySource};

    fn sample_ctx() -> PromptContext {
        PromptContext {
            workspace_name: "Coloc".into(),
            platform: "telegram".into(),
            member_count: 3,
            persona: "default".into(),
            language: "fr".into(),
            pinned_memories: vec![MemoryEntry {
                id: "m1".into(),
                key: Some("wifi".into()),
                value: "ABC123".into(),
                kind: MemoryKind::Fact,
                related_member: None,
                tags: vec![],
                source: MemorySource::ChatExplicit,
                confidence: 1.0,
                pinned: true,
                expires_at: None,
                created_by: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }],
            members: vec![MemberShort {
                display_name: "Alice".into(),
                role: "admin".into(),
            }],
            now_local: "2026-04-19 22:00".into(),
            timezone: "Europe/Paris".into(),
            proactive_mode: false,
            auto_memory: false,
            agent_calls_remaining: 950,
            agent_calls_quota: 1000,
            web_search_remaining: 50,
            web_search_quota: 50,
        }
    }

    #[test]
    fn prompt_contains_required_sections() {
        let p = build_system_prompt(&sample_ctx());
        assert!(p.contains("PERSONA:"));
        assert!(p.contains("MEMORY POLICY:"));
        assert!(p.contains("PINNED MEMORY:"));
        assert!(p.contains("MEMBERS:"));
        assert!(p.contains("RULES:"));
        assert!(p.contains("CURRENT DATETIME:"));
    }

    #[test]
    fn prompt_includes_workspace_name_and_pinned_memory() {
        let p = build_system_prompt(&sample_ctx());
        assert!(p.contains("Coloc"));
        assert!(p.contains("[wifi] ABC123"));
        assert!(p.contains("Alice (admin)"));
    }

    #[test]
    fn persona_default_includes_tagline() {
        let p = build_system_prompt(&sample_ctx());
        assert!(p.contains("Gets it done. No small talk."));
    }

    #[test]
    fn persona_formal_no_tagline() {
        let mut ctx = sample_ctx();
        ctx.persona = "formal".into();
        let p = build_system_prompt(&ctx);
        assert!(p.contains("Precise, polite"));
        assert!(!p.contains("No small talk"));
    }
}
