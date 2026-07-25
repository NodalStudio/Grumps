use grumps_core::todo::Priority;

#[derive(Debug, Clone, PartialEq)]
pub enum ParseResult {
    AddTodos(Vec<ParsedTodo>),
    CompleteTodos(Vec<String>),
    AddNote(ParsedNote),
    ListTodos(ListFilter),
    CompleteSingle(CompletionTarget),
    DeleteTodo(i64),
    ListNotes,
    SearchNotes(String),
    Help,
    WorkspaceLink,
    Status,
    /// Mention/DM text that matched no deterministic command. The caller routes
    /// this to the LLM agent as a last resort (or, with no agent available,
    /// falls back to creating a single todo from the text).
    Freeform(String),
    TaskCardReply(TaskCardAction),
    QuotedTodo,
    QuotedNote,
    Ignore,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedTodo {
    pub title: String,
    pub assignee_mention: Option<String>,
    pub deadline_text: Option<String>,
    pub priority: Priority,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedNote {
    pub title: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ListFilter {
    Open,
    All,
    Mine,
    Done,
    Assignee(String),
    Tag(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompletionTarget {
    BySeqNum(i64),
    ByText(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskCardAction {
    Done,
    Snooze(String),
    Edit(String),
    Reassign(String),
    ChangePriority(Priority),
    AddTag(String),
    Delete,
    ChangeStatus(String),
}

pub fn parse(
    text: &str,
    is_mention: bool,
    is_dm: bool,
    is_reply_to_bot: bool,
    has_quoted_message: bool,
) -> ParseResult {
    let trimmed = text.trim();
    if is_reply_to_bot {
        // Only recognized task-card verbs (done, reassign, …) act on the replied
        // card. Anything else (casual chatter, an explicit @grumps command)
        // falls through to normal parsing instead of being swallowed as a bogus
        // card action.
        if let Some(r) = crate::reply_parser::parse_reply(trimmed) {
            return r;
        }
    }
    if has_quoted_message && is_mention {
        if let Some(r) = crate::command_parser::try_parse_quoted_command(trimmed) {
            return r;
        }
    }
    if let Some(r) = crate::block_parser::try_parse_block(trimmed) {
        return r;
    }
    if is_mention || is_dm {
        return crate::command_parser::parse_mention(trimmed);
    }
    ParseResult::Ignore
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_ignored() {
        assert_eq!(
            parse("hello everyone", false, false, false, false),
            ParseResult::Ignore
        );
    }

    #[test]
    fn dm_routes_to_command_parser() {
        // A DM with no recognized command is unrecognized free text, routed to
        // the agent as a last resort.
        assert_eq!(
            parse("hello", false, true, false, false),
            ParseResult::Freeform("hello".to_string())
        );
    }

    #[test]
    fn reply_with_non_card_text_falls_through() {
        // Replying "thanks" to a bot message that isn't a task card must NOT be
        // interpreted as a card action; with no mention it is ignored.
        assert_eq!(
            parse("thanks", false, false, true, true),
            ParseResult::Ignore
        );
    }

    #[test]
    fn reply_with_card_verb_is_card_action() {
        assert_eq!(
            parse("done", false, false, true, true),
            ParseResult::TaskCardReply(TaskCardAction::Done)
        );
    }

    #[test]
    fn reply_with_explicit_command_falls_through() {
        // "@grumps list" typed as a reply is a command, not a reassignment.
        assert_eq!(
            parse("@grumps list", true, false, true, true),
            ParseResult::ListTodos(ListFilter::Open)
        );
    }
}
