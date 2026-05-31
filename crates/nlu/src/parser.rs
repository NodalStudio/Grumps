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
    ListFiles,
    Help,
    WorkspaceLink,
    Status,
    AddSingleTodo(ParsedTodo),
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
        return crate::reply_parser::parse_reply(trimmed);
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
        // DM with no known command → default action (currently Ignore from stub)
        let r = parse("hello", false, true, false, false);
        // Once command_parser is implemented this will be AddSingleTodo, for now it's Ignore from stub
        assert!(matches!(
            r,
            ParseResult::Ignore | ParseResult::AddSingleTodo(_)
        ));
    }
}
