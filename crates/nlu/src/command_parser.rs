use crate::parser::*;
use crate::entity;

pub fn parse_mention(text: &str) -> ParseResult {
    let clean = entity::strip_mention(text);
    let trimmed = clean.trim();
    if trimmed.is_empty() { return ParseResult::Status; }

    let lower = trimmed.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();

    match words[0] {
        "list" | "show" | "todos" => parse_list_command(&words[1..]),
        "done" | "complete" | "finished" => {
            if words.len() < 2 { return ParseResult::Status; }
            parse_done_command(trimmed[words[0].len()..].trim())
        }
        "delete" | "remove" => {
            if words.len() < 2 { return ParseResult::Help; }
            parse_delete_command(trimmed[words[0].len()..].trim())
        }
        "notes" => ParseResult::ListNotes,
        "note" | "pin" => {
            if words.len() < 2 { return ParseResult::ListNotes; }
            ParseResult::SearchNotes(trimmed[words[0].len()..].trim().to_string())
        }
        "files" | "file" => ParseResult::ListFiles,
        "help" | "?" => ParseResult::Help,
        "link" | "workspace" | "web" => ParseResult::WorkspaceLink,
        "status" | "summary" | "recap" => ParseResult::Status,
        _ => {
            let parsed = entity::extract_todo_from_line(trimmed);
            if parsed.title.is_empty() { ParseResult::Status } else { ParseResult::AddSingleTodo(parsed) }
        }
    }
}

fn parse_list_command(args: &[&str]) -> ParseResult {
    if args.is_empty() { return ParseResult::ListTodos(ListFilter::Open); }
    match args[0] {
        "all" => ParseResult::ListTodos(ListFilter::All),
        "mine" | "my" => ParseResult::ListTodos(ListFilter::Mine),
        "done" | "completed" | "finished" => ParseResult::ListTodos(ListFilter::Done),
        "open" => ParseResult::ListTodos(ListFilter::Open),
        _ if args[0].starts_with('@') => ParseResult::ListTodos(ListFilter::Assignee(args[0][1..].to_string())),
        _ if args[0].starts_with('#') => ParseResult::ListTodos(ListFilter::Tag(args[0][1..].to_string())),
        _ => ParseResult::ListTodos(ListFilter::Open),
    }
}

fn parse_done_command(text: &str) -> ParseResult {
    if text.starts_with('#') {
        if let Ok(n) = text[1..].parse::<i64>() {
            return ParseResult::CompleteSingle(CompletionTarget::BySeqNum(n));
        }
    }
    ParseResult::CompleteSingle(CompletionTarget::ByText(text.to_string()))
}

fn parse_delete_command(text: &str) -> ParseResult {
    let cleaned = text.trim_start_matches('#');
    if let Ok(n) = cleaned.parse::<i64>() { return ParseResult::DeleteTodo(n); }
    ParseResult::Help
}

pub fn try_parse_quoted_command(text: &str) -> Option<ParseResult> {
    let clean = entity::strip_mention(text).to_lowercase();
    let trimmed = clean.trim();
    match trimmed {
        "todo" | "add" | "task" => Some(ParseResult::QuotedTodo),
        "note" | "pin" | "save" => Some(ParseResult::QuotedNote),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_mention_returns_status() {
        assert_eq!(parse_mention("@grumps"), ParseResult::Status);
    }

    #[test]
    fn test_list_returns_list_open() {
        assert_eq!(parse_mention("@grumps list"), ParseResult::ListTodos(ListFilter::Open));
    }

    #[test]
    fn test_list_all() {
        assert_eq!(parse_mention("@grumps list all"), ParseResult::ListTodos(ListFilter::All));
    }

    #[test]
    fn test_list_mine() {
        assert_eq!(parse_mention("@grumps list mine"), ParseResult::ListTodos(ListFilter::Mine));
        assert_eq!(parse_mention("@grumps list my"), ParseResult::ListTodos(ListFilter::Mine));
    }

    #[test]
    fn test_list_done() {
        assert_eq!(parse_mention("@grumps list done"), ParseResult::ListTodos(ListFilter::Done));
        assert_eq!(parse_mention("@grumps list completed"), ParseResult::ListTodos(ListFilter::Done));
    }

    #[test]
    fn test_list_open() {
        assert_eq!(parse_mention("@grumps list open"), ParseResult::ListTodos(ListFilter::Open));
    }

    #[test]
    fn test_list_assignee() {
        assert_eq!(
            parse_mention("@grumps list @Pierre"),
            ParseResult::ListTodos(ListFilter::Assignee("pierre".to_string()))
        );
    }

    #[test]
    fn test_list_tag() {
        assert_eq!(
            parse_mention("@grumps list #urgent"),
            ParseResult::ListTodos(ListFilter::Tag("urgent".to_string()))
        );
    }

    #[test]
    fn test_show_alias() {
        assert_eq!(parse_mention("@grumps show"), ParseResult::ListTodos(ListFilter::Open));
    }

    #[test]
    fn test_todos_alias() {
        assert_eq!(parse_mention("@grumps todos"), ParseResult::ListTodos(ListFilter::Open));
    }

    #[test]
    fn test_done_seq_num() {
        assert_eq!(
            parse_mention("@grumps done #42"),
            ParseResult::CompleteSingle(CompletionTarget::BySeqNum(42))
        );
    }

    #[test]
    fn test_done_by_text() {
        assert_eq!(
            parse_mention("@grumps done bread"),
            ParseResult::CompleteSingle(CompletionTarget::ByText("bread".to_string()))
        );
    }

    #[test]
    fn test_complete_seq_num() {
        assert_eq!(
            parse_mention("@grumps complete #5"),
            ParseResult::CompleteSingle(CompletionTarget::BySeqNum(5))
        );
    }

    #[test]
    fn test_delete_seq_num() {
        assert_eq!(parse_mention("@grumps delete #42"), ParseResult::DeleteTodo(42));
    }

    #[test]
    fn test_remove_seq_num() {
        assert_eq!(parse_mention("@grumps remove #42"), ParseResult::DeleteTodo(42));
    }

    #[test]
    fn test_notes() {
        assert_eq!(parse_mention("@grumps notes"), ParseResult::ListNotes);
    }

    #[test]
    fn test_note_search() {
        assert_eq!(
            parse_mention("@grumps note wifi"),
            ParseResult::SearchNotes("wifi".to_string())
        );
    }

    #[test]
    fn test_files() {
        assert_eq!(parse_mention("@grumps files"), ParseResult::ListFiles);
    }

    #[test]
    fn test_help() {
        assert_eq!(parse_mention("@grumps help"), ParseResult::Help);
    }

    #[test]
    fn test_question_mark_help() {
        assert_eq!(parse_mention("@grumps ?"), ParseResult::Help);
    }

    #[test]
    fn test_link() {
        assert_eq!(parse_mention("@grumps link"), ParseResult::WorkspaceLink);
    }

    #[test]
    fn test_workspace() {
        assert_eq!(parse_mention("@grumps workspace"), ParseResult::WorkspaceLink);
    }

    #[test]
    fn test_status() {
        assert_eq!(parse_mention("@grumps status"), ParseResult::Status);
        assert_eq!(parse_mention("@grumps summary"), ParseResult::Status);
    }

    #[test]
    fn test_default_add_todo() {
        let result = parse_mention("@grumps buy bread @Bob");
        match result {
            ParseResult::AddSingleTodo(todo) => {
                assert_eq!(todo.title, "buy bread");
                assert_eq!(todo.assignee_mention, Some("Bob".to_string()));
            }
            other => panic!("Expected AddSingleTodo, got {:?}", other),
        }
    }

    #[test]
    fn test_quoted_todo() {
        assert_eq!(
            try_parse_quoted_command("@grumps todo"),
            Some(ParseResult::QuotedTodo)
        );
    }

    #[test]
    fn test_quoted_add() {
        assert_eq!(
            try_parse_quoted_command("@grumps add"),
            Some(ParseResult::QuotedTodo)
        );
    }

    #[test]
    fn test_quoted_task() {
        assert_eq!(
            try_parse_quoted_command("@grumps task"),
            Some(ParseResult::QuotedTodo)
        );
    }

    #[test]
    fn test_quoted_note() {
        assert_eq!(
            try_parse_quoted_command("@grumps note"),
            Some(ParseResult::QuotedNote)
        );
    }

    #[test]
    fn test_quoted_pin() {
        assert_eq!(
            try_parse_quoted_command("@grumps pin"),
            Some(ParseResult::QuotedNote)
        );
    }

    #[test]
    fn test_quoted_save() {
        assert_eq!(
            try_parse_quoted_command("@grumps save"),
            Some(ParseResult::QuotedNote)
        );
    }

    #[test]
    fn test_quoted_unknown_returns_none() {
        assert_eq!(try_parse_quoted_command("@grumps something"), None);
    }
}
