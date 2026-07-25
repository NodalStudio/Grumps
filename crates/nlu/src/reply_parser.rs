use crate::parser::{ParseResult, TaskCardAction};
use grumps_core::todo::Priority;

/// True if `s` looks like a bounded card-reply capture: non-empty, at most
/// 3 words, and free of sentence punctuation. Longer or punctuated captures
/// ("Bob can you take this today?", "I think we should wait for the client
/// to answer") are ordinary chat that happens to start with `@`/`#`/
/// `status:`, not a reassignment/tag/status verb — the caller falls through
/// to normal parsing instead of swallowing the whole sentence.
fn is_bounded_capture(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.contains(['.', ',', '!', '?', ';']) {
        return false;
    }
    trimmed.split_whitespace().count() <= 3
}

/// Parse a reply to a bot task card. Returns `Some` only for recognized card
/// verbs; unrecognized text (casual chatter) and explicit `@grumps` commands
/// return `None` so the caller can fall through to normal parsing instead of
/// swallowing the message as a bogus card action.
pub fn parse_reply(text: &str) -> Option<ParseResult> {
    let lower = text.trim().to_lowercase();
    let original = text.trim();
    let action = match lower.as_str() {
        // "ok" is the default human ack to *any* bot message, not a
        // deliberate "mark this task done" — dropped as a Done synonym.
        "done" | "finished" | "complete" | "fait" | "fini" => TaskCardAction::Done,
        "cancel" | "delete" | "remove" | "supprimer" => TaskCardAction::Delete,
        "snooze" | "reporter" => TaskCardAction::Snooze(String::new()),
        "!high" | "!!!" => TaskCardAction::ChangePriority(Priority::High),
        "!low" => TaskCardAction::ChangePriority(Priority::Low),
        _ if lower.starts_with("edit ") => TaskCardAction::Edit(original[5..].trim().into()),
        _ if lower.starts_with("snooze ") => TaskCardAction::Snooze(original[7..].trim().into()),
        _ if lower.starts_with("reporter ") => TaskCardAction::Snooze(original[9..].trim().into()),
        // A reply that mentions the bot is an explicit command, not a
        // reassignment — let normal mention parsing handle it.
        _ if lower.starts_with("@grumps") => return None,
        _ if lower.starts_with('@') && original.len() > 1 => {
            let captured = original[1..].trim();
            if !is_bounded_capture(captured) {
                return None;
            }
            TaskCardAction::Reassign(captured.into())
        }
        _ if lower.starts_with('#') && original.len() > 1 => {
            let captured = original[1..].trim();
            if !is_bounded_capture(captured) {
                return None;
            }
            TaskCardAction::AddTag(captured.into())
        }
        _ if lower.starts_with("status:") || lower.starts_with("status ") => {
            let captured = original[7..].trim();
            if !is_bounded_capture(captured) {
                return None;
            }
            TaskCardAction::ChangeStatus(captured.into())
        }
        // Not a recognized card verb — fall through to normal parsing.
        _ => return None,
    };
    Some(ParseResult::TaskCardReply(action))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::TaskCardAction;
    use grumps_core::todo::Priority;

    fn action(text: &str) -> TaskCardAction {
        match parse_reply(text) {
            Some(ParseResult::TaskCardReply(a)) => a,
            other => panic!("Expected Some(TaskCardReply), got {:?}", other),
        }
    }

    #[test]
    fn done_keywords() {
        for kw in &["done", "finished", "complete", "fait", "fini"] {
            assert_eq!(action(kw), TaskCardAction::Done, "failed for '{kw}'");
        }
    }

    #[test]
    fn bare_ok_no_longer_completes() {
        // "ok" is the default human ack to *any* bot message — it must not
        // silently mark the task done.
        assert_eq!(parse_reply("ok"), None);
        assert_eq!(parse_reply("OK"), None);
        assert_eq!(parse_reply("  ok  "), None);
    }

    #[test]
    fn done_case_insensitive() {
        assert_eq!(action("DONE"), TaskCardAction::Done);
        assert_eq!(action("Done"), TaskCardAction::Done);
        assert_eq!(action("  done  "), TaskCardAction::Done);
    }

    #[test]
    fn cancel_keywords() {
        for kw in &["cancel", "delete", "remove", "supprimer"] {
            assert_eq!(action(kw), TaskCardAction::Delete, "failed for '{kw}'");
        }
    }

    #[test]
    fn snooze_bare() {
        assert_eq!(action("snooze"), TaskCardAction::Snooze(String::new()));
        assert_eq!(action("reporter"), TaskCardAction::Snooze(String::new()));
    }

    #[test]
    fn snooze_with_arg() {
        assert_eq!(
            action("snooze tomorrow"),
            TaskCardAction::Snooze("tomorrow".into())
        );
        assert_eq!(
            action("snooze 2026-08-01"),
            TaskCardAction::Snooze("2026-08-01".into())
        );
        assert_eq!(
            action("reporter demain"),
            TaskCardAction::Snooze("demain".into())
        );
    }

    #[test]
    fn snooze_case_insensitive() {
        assert_eq!(action("SNOOZE"), TaskCardAction::Snooze(String::new()));
        assert_eq!(
            action("Snooze Tomorrow"),
            TaskCardAction::Snooze("Tomorrow".into())
        );
    }

    #[test]
    fn priority_high() {
        assert_eq!(
            action("!high"),
            TaskCardAction::ChangePriority(Priority::High)
        );
        assert_eq!(
            action("!!!"),
            TaskCardAction::ChangePriority(Priority::High)
        );
    }

    #[test]
    fn priority_low() {
        assert_eq!(
            action("!low"),
            TaskCardAction::ChangePriority(Priority::Low)
        );
    }

    #[test]
    fn edit_command() {
        assert_eq!(
            action("edit New title for the task"),
            TaskCardAction::Edit("New title for the task".into())
        );
        // Preserves original casing after "edit "
        assert_eq!(
            action("edit MyTitle"),
            TaskCardAction::Edit("MyTitle".into())
        );
    }

    #[test]
    fn edit_case_insensitive_prefix() {
        assert_eq!(
            action("EDIT New title"),
            TaskCardAction::Edit("New title".into())
        );
    }

    #[test]
    fn reassign() {
        assert_eq!(action("@Alice"), TaskCardAction::Reassign("Alice".into()));
        assert_eq!(
            action("@Bob Smith"),
            TaskCardAction::Reassign("Bob Smith".into())
        );
    }

    #[test]
    fn add_tag() {
        assert_eq!(action("#urgent"), TaskCardAction::AddTag("urgent".into()));
        assert_eq!(action("#backend"), TaskCardAction::AddTag("backend".into()));
    }

    #[test]
    fn change_status_colon() {
        assert_eq!(
            action("status: blocked"),
            TaskCardAction::ChangeStatus("blocked".into())
        );
    }

    #[test]
    fn change_status_space() {
        assert_eq!(
            action("status in_progress"),
            TaskCardAction::ChangeStatus("in_progress".into())
        );
    }

    // --- bounded captures (issue #21): over ~3 words or sentence
    // punctuation falls through to normal parsing instead of swallowing a
    // whole sentence as a reassign/tag/status verb. ---

    #[test]
    fn reassign_unbounded_falls_through() {
        assert_eq!(parse_reply("@Bob can you take this today?"), None);
    }

    #[test]
    fn reassign_bounded_still_works() {
        // Sanity: the 3-word ceiling and the punctuation check don't reject
        // legitimate short reassignments.
        assert_eq!(
            action("@Bob Smith Jr"),
            TaskCardAction::Reassign("Bob Smith Jr".into())
        );
    }

    #[test]
    fn add_tag_still_works() {
        assert_eq!(action("#urgent"), TaskCardAction::AddTag("urgent".into()));
    }

    #[test]
    fn add_tag_unbounded_falls_through() {
        assert_eq!(
            parse_reply("#this is definitely not a real tag, is it?"),
            None
        );
    }

    #[test]
    fn change_status_still_works() {
        assert_eq!(
            action("status: blocked"),
            TaskCardAction::ChangeStatus("blocked".into())
        );
    }

    #[test]
    fn change_status_unbounded_falls_through() {
        assert_eq!(
            parse_reply("status: I think we should wait for the client to answer"),
            None
        );
    }

    #[test]
    fn unrecognized_text_falls_through() {
        // Casual chatter and bare date words are no longer swallowed as a card
        // action — they return None so the caller falls through to normal
        // parsing (or ignores the message).
        for text in &[
            "tomorrow",
            "monday",
            "in 2h",
            "next week",
            "random text",
            "thanks",
        ] {
            assert_eq!(parse_reply(text), None, "failed for '{text}'");
        }
    }

    #[test]
    fn explicit_mention_command_falls_through() {
        // "@grumps list" as a reply is a command, not a reassignment.
        assert_eq!(parse_reply("@grumps list"), None);
    }

    #[test]
    fn bare_at_sign_falls_through() {
        assert_eq!(parse_reply("@"), None);
    }

    #[test]
    fn bare_hash_falls_through() {
        assert_eq!(parse_reply("#"), None);
    }
}
