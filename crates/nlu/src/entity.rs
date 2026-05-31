use crate::parser::ParsedTodo;
use grumps_core::todo::Priority;

pub fn extract_todo_from_line(line: &str) -> ParsedTodo {
    let mut assignee: Option<String> = None;
    let mut priority = Priority::Normal;
    let mut tags: Vec<String> = Vec::new();
    let mut deadline_text: Option<String> = None;
    let mut title_parts: Vec<String> = Vec::new();
    let words: Vec<&str> = line.split_whitespace().collect();
    let mut i = 0;
    let mut in_deadline = false;
    let mut deadline_parts: Vec<&str> = Vec::new();

    while i < words.len() {
        let w = words[i];
        if !in_deadline
            && (w.eq_ignore_ascii_case("before")
                || w.eq_ignore_ascii_case("by")
                || w.eq_ignore_ascii_case("for"))
        {
            if i + 1 < words.len() {
                let next = words[i + 1].to_lowercase();
                let is_date = matches!(
                    next.as_str(),
                    "monday"
                        | "tuesday"
                        | "wednesday"
                        | "thursday"
                        | "friday"
                        | "saturday"
                        | "sunday"
                        | "tomorrow"
                        | "today"
                        | "tonight"
                        | "next"
                        | "end"
                ) || next
                    .chars()
                    .next()
                    .map(|c| c.is_ascii_digit())
                    .unwrap_or(false);
                if is_date {
                    in_deadline = true;
                    i += 1;
                    continue;
                }
            }
            title_parts.push(w.to_string());
        } else if in_deadline {
            if w.starts_with('@') || w.starts_with('#') || w.starts_with('!') {
                in_deadline = false;
                deadline_text = Some(deadline_parts.join(" "));
                deadline_parts.clear();
                continue;
            }
            deadline_parts.push(w);
        } else if w.starts_with('@') && w.len() > 1 {
            if assignee.is_none() {
                assignee = Some(w[1..].to_string());
            }
        } else if w == "!high" || w == "!!!" {
            priority = Priority::High;
        } else if w == "!low" {
            priority = Priority::Low;
        } else if w.starts_with('#') && w.len() > 1 {
            tags.push(w[1..].to_string());
        } else {
            title_parts.push(w.to_string());
        }
        i += 1;
    }
    if !deadline_parts.is_empty() {
        deadline_text = Some(deadline_parts.join(" "));
    }
    ParsedTodo {
        title: title_parts.join(" "),
        assignee_mention: assignee,
        deadline_text,
        priority,
        tags,
    }
}

pub fn strip_mention(text: &str) -> String {
    let lower = text.to_lowercase();
    if lower.starts_with("@grumps ") {
        return text[8..].trim().to_string();
    }
    if lower.starts_with("@grumps") {
        return text[7..].trim().to_string();
    }
    if let Some(pos) = lower.find("@grumps") {
        let before = &text[..pos];
        let after = if pos + 7 < text.len() {
            &text[pos + 7..]
        } else {
            ""
        };
        let before = before.trim_end();
        let after = after.trim_start();
        return if before.is_empty() || after.is_empty() {
            format!("{}{}", before, after).trim().to_string()
        } else {
            format!("{} {}", before, after)
        };
    }
    text.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use grumps_core::todo::Priority;

    #[test]
    fn plain_line() {
        let t = extract_todo_from_line("Buy toilet paper");
        assert_eq!(t.title, "Buy toilet paper");
        assert!(t.assignee_mention.is_none());
        assert_eq!(t.priority, Priority::Normal);
        assert!(t.tags.is_empty());
        assert!(t.deadline_text.is_none());
    }

    #[test]
    fn with_assignee() {
        let t = extract_todo_from_line("Follow up with client @Pierre");
        assert_eq!(t.title, "Follow up with client");
        assert_eq!(t.assignee_mention, Some("Pierre".into()));
    }

    #[test]
    fn with_priority_high() {
        let t = extract_todo_from_line("Fix bug !high");
        assert_eq!(t.title, "Fix bug");
        assert_eq!(t.priority, Priority::High);
    }

    #[test]
    fn with_priority_bang() {
        let t = extract_todo_from_line("Fix bug !!!");
        assert_eq!(t.priority, Priority::High);
    }

    #[test]
    fn with_priority_low() {
        let t = extract_todo_from_line("Clean desk !low");
        assert_eq!(t.priority, Priority::Low);
    }

    #[test]
    fn with_tags() {
        let t = extract_todo_from_line("Fix CSS #frontend #urgent");
        assert_eq!(t.title, "Fix CSS");
        assert_eq!(t.tags, vec!["frontend", "urgent"]);
    }

    #[test]
    fn with_everything() {
        let t = extract_todo_from_line("Ship project @Pierre !high #sales #client");
        assert_eq!(t.title, "Ship project");
        assert_eq!(t.assignee_mention, Some("Pierre".into()));
        assert_eq!(t.priority, Priority::High);
        assert_eq!(t.tags, vec!["sales", "client"]);
    }

    #[test]
    fn deadline_before() {
        let t = extract_todo_from_line("Buy gifts before friday @Alice");
        assert_eq!(t.title, "Buy gifts");
        assert_eq!(t.deadline_text, Some("friday".into()));
        assert_eq!(t.assignee_mention, Some("Alice".into()));
    }

    #[test]
    fn deadline_by_tomorrow() {
        let t = extract_todo_from_line("Finish report by tomorrow");
        assert_eq!(t.title, "Finish report");
        assert_eq!(t.deadline_text, Some("tomorrow".into()));
    }

    #[test]
    fn deadline_for_friday() {
        let t = extract_todo_from_line("Book restaurant for friday @Bob #dinner");
        assert_eq!(t.title, "Book restaurant");
        assert_eq!(t.deadline_text, Some("friday".into()));
        assert_eq!(t.assignee_mention, Some("Bob".into()));
        assert_eq!(t.tags, vec!["dinner"]);
    }

    #[test]
    fn for_non_date_stays_in_title() {
        let t = extract_todo_from_line("Buy gift for mom");
        assert_eq!(t.title, "Buy gift for mom");
        assert!(t.deadline_text.is_none());
    }

    #[test]
    fn strip_mention_start() {
        assert_eq!(strip_mention("@grumps buy bread"), "buy bread");
    }

    #[test]
    fn strip_mention_case() {
        assert_eq!(strip_mention("@Grumps buy bread"), "buy bread");
    }

    #[test]
    fn strip_mention_middle() {
        assert_eq!(strip_mention("hey @grumps buy bread"), "hey buy bread");
    }

    #[test]
    fn strip_mention_none() {
        assert_eq!(strip_mention("hello world"), "hello world");
    }

    #[test]
    fn strip_mention_only() {
        assert_eq!(strip_mention("@grumps"), "");
    }
}
