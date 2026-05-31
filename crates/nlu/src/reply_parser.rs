use crate::parser::{ParseResult, TaskCardAction};
use grumps_core::todo::Priority;

pub fn parse_reply(text: &str) -> ParseResult {
    let lower = text.trim().to_lowercase();
    let original = text.trim();
    let action = match lower.as_str() {
        "done" | "finished" | "complete" | "ok" | "fait" | "fini" => TaskCardAction::Done,
        "cancel" | "delete" | "remove" | "supprimer" => TaskCardAction::Delete,
        "!high" | "!!!" => TaskCardAction::ChangePriority(Priority::High),
        "!low" => TaskCardAction::ChangePriority(Priority::Low),
        _ if lower.starts_with("edit ") => TaskCardAction::Edit(original[5..].trim().into()),
        _ if lower.starts_with('@') && original.len() > 1 => {
            TaskCardAction::Reassign(original[1..].trim().into())
        }
        _ if lower.starts_with('#') && original.len() > 1 => {
            TaskCardAction::AddTag(original[1..].trim().into())
        }
        _ if lower.starts_with("status:") || lower.starts_with("status ") => {
            TaskCardAction::ChangeStatus(original[7..].trim().into())
        }
        _ => TaskCardAction::Snooze(original.into()),
    };
    ParseResult::TaskCardReply(action)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::TaskCardAction;
    use grumps_core::todo::Priority;

    fn action(text: &str) -> TaskCardAction {
        match parse_reply(text) {
            ParseResult::TaskCardReply(a) => a,
            other => panic!("Expected TaskCardReply, got {:?}", other),
        }
    }

    #[test]
    fn done_keywords() {
        for kw in &["done", "finished", "complete", "ok", "fait", "fini"] {
            assert_eq!(action(kw), TaskCardAction::Done, "failed for '{kw}'");
        }
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

    #[test]
    fn snooze_fallback() {
        for text in &["tomorrow", "monday", "in 2h", "next week", "random text"] {
            assert_eq!(
                action(text),
                TaskCardAction::Snooze((*text).into()),
                "failed for '{text}'"
            );
        }
    }

    #[test]
    fn snooze_preserves_original_text() {
        assert_eq!(action("In 2H"), TaskCardAction::Snooze("In 2H".into()));
    }

    #[test]
    fn bare_at_sign_is_snooze() {
        assert_eq!(action("@"), TaskCardAction::Snooze("@".into()));
    }

    #[test]
    fn bare_hash_is_snooze() {
        assert_eq!(action("#"), TaskCardAction::Snooze("#".into()));
    }
}
