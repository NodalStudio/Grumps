use crate::entity;
use crate::parser::{ParseResult, ParsedNote};

pub fn try_parse_block(text: &str) -> Option<ParseResult> {
    let upper = text.to_uppercase();
    if upper.starts_with("TODO:") {
        return parse_todo_block(&text[5..]);
    }
    if upper.starts_with("DONE:") {
        return parse_done_block(&text[5..]);
    }
    if upper.starts_with("NOTE [") {
        return parse_named_note(text);
    }
    if upper.starts_with("NOTE:") {
        return parse_note(&text[5..]);
    }
    None
}

fn parse_todo_block(body: &str) -> Option<ParseResult> {
    let items = parse_list_items(body);
    if items.is_empty() {
        return None;
    }
    let todos = items
        .iter()
        .map(|l| entity::extract_todo_from_line(l))
        .collect();
    Some(ParseResult::AddTodos(todos))
}

fn parse_done_block(body: &str) -> Option<ParseResult> {
    let items = parse_list_items(body);
    if items.is_empty() {
        return None;
    }
    Some(ParseResult::CompleteTodos(items))
}

fn parse_note(body: &str) -> Option<ParseResult> {
    let content = body.trim().to_string();
    if content.is_empty() {
        return None;
    }
    let title = content
        .lines()
        .next()
        .filter(|l| l.trim().len() <= 60)
        .map(|l| l.trim().to_string());
    Some(ParseResult::AddNote(ParsedNote { title, content }))
}

fn parse_named_note(text: &str) -> Option<ParseResult> {
    let rest = &text[6..]; // skip "NOTE ["
    let end = rest.find(']')?;
    let title = rest[..end].trim().to_string();
    let content = rest[end + 1..].trim_start_matches(':').trim().to_string();
    if content.is_empty() {
        return None;
    }
    Some(ParseResult::AddNote(ParsedNote {
        title: Some(title),
        content,
    }))
}

fn parse_list_items(body: &str) -> Vec<String> {
    body.lines()
        .map(|l| {
            l.trim()
                .trim_start_matches(|c: char| "•-*·◦▪▸►".contains(c) || c.is_whitespace())
                .trim()
                .to_string()
        })
        .filter(|l| !l.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ParseResult;
    use grumps_core::todo::Priority;

    // 1. TODO block with bullet points (•)
    #[test]
    fn todo_bullet_points() {
        let r = try_parse_block("TODO:\n• Buy milk\n• Call dentist");
        match r {
            Some(ParseResult::AddTodos(todos)) => {
                assert_eq!(todos.len(), 2);
                assert_eq!(todos[0].title, "Buy milk");
                assert_eq!(todos[1].title, "Call dentist");
            }
            _ => panic!("expected AddTodos"),
        }
    }

    // 2. TODO block with dashes (-)
    #[test]
    fn todo_dashes() {
        let r = try_parse_block("TODO:\n- Fix login bug\n- Update docs");
        match r {
            Some(ParseResult::AddTodos(todos)) => {
                assert_eq!(todos.len(), 2);
                assert_eq!(todos[0].title, "Fix login bug");
                assert_eq!(todos[1].title, "Update docs");
            }
            _ => panic!("expected AddTodos"),
        }
    }

    // 3. TODO block with asterisks (*)
    #[test]
    fn todo_asterisks() {
        let r = try_parse_block("TODO:\n* Write tests\n* Deploy");
        match r {
            Some(ParseResult::AddTodos(todos)) => {
                assert_eq!(todos.len(), 2);
                assert_eq!(todos[0].title, "Write tests");
                assert_eq!(todos[1].title, "Deploy");
            }
            _ => panic!("expected AddTodos"),
        }
    }

    // 4. TODO block with entities (@Pierre !high #tag)
    #[test]
    fn todo_with_entities() {
        let r = try_parse_block("TODO:\n- Ship feature @Pierre !high #backend");
        match r {
            Some(ParseResult::AddTodos(todos)) => {
                assert_eq!(todos.len(), 1);
                assert_eq!(todos[0].title, "Ship feature");
                assert_eq!(todos[0].assignee_mention, Some("Pierre".into()));
                assert_eq!(todos[0].priority, Priority::High);
                assert_eq!(todos[0].tags, vec!["backend"]);
            }
            _ => panic!("expected AddTodos"),
        }
    }

    // 5. TODO block case insensitive (todo:, Todo:)
    #[test]
    fn todo_case_insensitive() {
        let r1 = try_parse_block("todo:\n- Do something");
        let r2 = try_parse_block("Todo:\n- Do something");
        assert!(matches!(r1, Some(ParseResult::AddTodos(_))));
        assert!(matches!(r2, Some(ParseResult::AddTodos(_))));
    }

    // 6. TODO block empty → None
    #[test]
    fn todo_empty_none() {
        assert!(try_parse_block("TODO:").is_none());
        assert!(try_parse_block("TODO:\n   \n  ").is_none());
    }

    // 7. DONE block with items
    #[test]
    fn done_with_items() {
        let r = try_parse_block("DONE:\n- Fix login\n- Update readme");
        match r {
            Some(ParseResult::CompleteTodos(items)) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0], "Fix login");
                assert_eq!(items[1], "Update readme");
            }
            _ => panic!("expected CompleteTodos"),
        }
    }

    // 8. DONE block empty → None
    #[test]
    fn done_empty_none() {
        assert!(try_parse_block("DONE:").is_none());
    }

    // 9. NOTE simple
    #[test]
    fn note_simple() {
        let r = try_parse_block("NOTE: Remember to water the plants");
        match r {
            Some(ParseResult::AddNote(note)) => {
                assert_eq!(note.content, "Remember to water the plants");
            }
            _ => panic!("expected AddNote"),
        }
    }

    // 10. NOTE with title extraction (short first line)
    #[test]
    fn note_title_extraction_short_first_line() {
        let r = try_parse_block("NOTE:\nShort title\nThis is the rest of the content.");
        match r {
            Some(ParseResult::AddNote(note)) => {
                assert_eq!(note.title, Some("Short title".into()));
                assert!(note.content.contains("Short title"));
            }
            _ => panic!("expected AddNote"),
        }
    }

    // NOTE: first line too long → no title
    #[test]
    fn note_long_first_line_no_title() {
        let long = "NOTE:\n".to_string() + &"x".repeat(61);
        let r = try_parse_block(&long);
        match r {
            Some(ParseResult::AddNote(note)) => {
                assert!(note.title.is_none());
            }
            _ => panic!("expected AddNote"),
        }
    }

    // 11. NOTE named: NOTE [wifi]: content
    #[test]
    fn note_named() {
        let r = try_parse_block("NOTE [wifi]: password is abc123");
        match r {
            Some(ParseResult::AddNote(note)) => {
                assert_eq!(note.title, Some("wifi".into()));
                assert_eq!(note.content, "password is abc123");
            }
            _ => panic!("expected AddNote"),
        }
    }

    // 12. NOTE empty → None
    #[test]
    fn note_empty_none() {
        assert!(try_parse_block("NOTE:").is_none());
        assert!(try_parse_block("NOTE:   ").is_none());
    }

    // 13. Not a block (plain text) → None
    #[test]
    fn plain_text_none() {
        assert!(try_parse_block("hello everyone").is_none());
        assert!(try_parse_block("What should we do today?").is_none());
    }

    // 14. Multi-line note content
    #[test]
    fn note_multiline_content() {
        let r = try_parse_block("NOTE:\nLine one\nLine two\nLine three");
        match r {
            Some(ParseResult::AddNote(note)) => {
                assert!(note.content.contains("Line one"));
                assert!(note.content.contains("Line two"));
                assert!(note.content.contains("Line three"));
            }
            _ => panic!("expected AddNote"),
        }
    }

    // 15. TODO single item (no bullet, just text after TODO:)
    #[test]
    fn todo_single_item_no_bullet() {
        let r = try_parse_block("TODO: Buy groceries");
        match r {
            Some(ParseResult::AddTodos(todos)) => {
                assert_eq!(todos.len(), 1);
                assert_eq!(todos[0].title, "Buy groceries");
            }
            _ => panic!("expected AddTodos"),
        }
    }
}
