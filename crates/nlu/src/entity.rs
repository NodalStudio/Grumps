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

    while i < words.len() {
        let w = words[i];
        if deadline_text.is_none()
            && (w.eq_ignore_ascii_case("before")
                || w.eq_ignore_ascii_case("by")
                || w.eq_ignore_ascii_case("for")
                || w.eq_ignore_ascii_case("avant")
                || w.eq_ignore_ascii_case("pour")
                || w.eq_ignore_ascii_case("d'ici"))
        {
            if let Some((expr, next_i)) = try_capture_date_expr(&words, i + 1) {
                deadline_text = Some(expr);
                i = next_i;
                continue;
            }
            // Either the next word isn't a date word, or the date phrase is
            // trailed by ordinary prose ("sunday roast", "vendredi soir") —
            // abstain and leave the trigger word (and whatever follows,
            // handled by the default branch on later iterations) in the
            // title instead of guessing at a phantom deadline.
            title_parts.push(w.to_string());
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

    // No trigger word ("before"/"avant"/...) fired, but the line still ends
    // with a bare date word ("appeler le plombier demain"). Pull it out of
    // the title so the two extraction paths (this one and the agent path,
    // which already resolves such trailing words) agree. Skip when the
    // word is a possessive/genitive reference ("le journal de demain")
    // rather than a deadline.
    if deadline_text.is_none() {
        if let Some(last) = title_parts.last() {
            if is_trailing_deadline_word(last) {
                let preceded_by_possessive = title_parts.len() >= 2
                    && matches!(
                        title_parts[title_parts.len() - 2].to_lowercase().as_str(),
                        "de" | "du" | "of"
                    );
                if !preceded_by_possessive {
                    deadline_text = title_parts.pop();
                }
            }
        }
    }

    ParsedTodo {
        title: title_parts.join(" "),
        assignee_mention: assignee,
        deadline_text,
        priority,
        tags,
    }
}

/// True if `w` (case-insensitive) can open (or continue) a date expression
/// after a trigger word ("before", "avant", ...): a weekday, a relative day
/// word, "next"/"end" (multi-word continuations like "next friday"), or
/// anything starting with a digit (`2026-08-01`).
fn is_date_word(w: &str) -> bool {
    let lower = w.to_lowercase();
    matches!(
        lower.as_str(),
        "monday"
            | "tuesday"
            | "wednesday"
            | "thursday"
            | "friday"
            | "saturday"
            | "sunday"
            | "lundi"
            | "mardi"
            | "mercredi"
            | "jeudi"
            | "vendredi"
            | "samedi"
            | "dimanche"
            | "tomorrow"
            | "today"
            | "tonight"
            | "demain"
            | "aujourd'hui"
            | "aujourdhui"
            | "next"
            | "end"
    ) || lower
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
}

/// Attempts to capture a contiguous run of date words in `words` starting at
/// `start`. Returns `(joined expression, index right after the expression)`
/// only when the expression is either terminal (end of message) or
/// immediately followed by a meta token (`@`/`#`/`!`) — anything else means
/// the apparent date phrase is actually trailed by ordinary prose ("sunday
/// roast", "vendredi soir"), so the caller should abstain and treat it as
/// plain title text instead of guessing at a deadline.
fn try_capture_date_expr(words: &[&str], start: usize) -> Option<(String, usize)> {
    if start >= words.len() || !is_date_word(words[start]) {
        return None;
    }
    let mut end = start + 1;
    while end < words.len() && is_date_word(words[end]) {
        end += 1;
    }
    let terminal_or_meta = end >= words.len()
        || words[end].starts_with('@')
        || words[end].starts_with('#')
        || words[end].starts_with('!');
    if !terminal_or_meta {
        return None;
    }
    Some((words[start..end].join(" "), end))
}

/// If `text` is eligible to be treated as an explicit `@grumps <phrase>` /
/// `@heygrumpsbot <phrase>` fast command (silence, unsilence, "remember
/// that ..."), returns the message with the leading mention token stripped
/// and outer whitespace trimmed — original casing preserved, so the caller
/// can match/slice it directly. Returns `None` when the message doesn't
/// literally *start* with the mention (an embedded "please @grumps quiet"
/// no longer counts — the caller then requires the remainder to match a
/// known phrase exactly, so stray text anywhere disqualifies it either
/// way) or when it's a `TODO:`/`DONE:`/`NOTE:` block — a bullet line inside
/// one that happens to contain "@grumps quiet" must be parsed as a todo,
/// never as a silence toggle.
pub fn strip_fast_command_mention(text: &str) -> Option<&str> {
    let trimmed = text.trim_start();
    if crate::block_parser::is_block_prefix(trimmed) {
        return None;
    }
    for mention in ["@heygrumpsbot", "@grumps"] {
        if let Some(head) = trimmed.get(..mention.len()) {
            if head.eq_ignore_ascii_case(mention) {
                return Some(trimmed[mention.len()..].trim());
            }
        }
    }
    None
}

/// True if `w` (case-insensitive) is a bare date word that can stand as a
/// trailing deadline with no trigger word ("...demain", "...friday").
fn is_trailing_deadline_word(w: &str) -> bool {
    let lower = w.to_lowercase();
    matches!(
        lower.as_str(),
        "today"
            | "tonight"
            | "tomorrow"
            | "demain"
            | "aujourd'hui"
            | "aujourdhui"
            | "monday"
            | "tuesday"
            | "wednesday"
            | "thursday"
            | "friday"
            | "saturday"
            | "sunday"
            | "lundi"
            | "mardi"
            | "mercredi"
            | "jeudi"
            | "vendredi"
            | "samedi"
            | "dimanche"
    )
}

/// Case-insensitive search for an ASCII `needle`, returning a byte offset into
/// `text`. Unlike `text.to_lowercase().find()`, the offset is always valid to
/// slice `text` with: lowercasing can change byte length (e.g. Turkish `İ`),
/// so an offset into a lowercased copy may land mid-codepoint in the original.
fn find_ascii_ci(text: &str, needle: &str) -> Option<usize> {
    let (hay, nee) = (text.as_bytes(), needle.as_bytes());
    if nee.is_empty() || hay.len() < nee.len() {
        return None;
    }
    (0..=hay.len() - nee.len()).find(|&i| {
        hay[i..i + nee.len()]
            .iter()
            .zip(nee)
            .all(|(b, n)| b.eq_ignore_ascii_case(n))
    })
}

/// Resolve a relative deadline hint ("friday", "demain", an explicit
/// `YYYY-MM-DD`, ...) to a civil date, given `today` (the caller computes
/// `today` in the workspace timezone; this function is pure and tz-agnostic
/// otherwise). Returns `None` for anything unrecognized — the caller then
/// keeps the raw hint for display and skips persisting a deadline.
///
/// Accepted forms (case-insensitive):
/// - `YYYY-MM-DD` — passed through unchanged.
/// - `today` / `tonight` / `aujourd'hui` / `aujourdhui` — `today`.
/// - `tomorrow` / `demain` — `today + 1 day`.
/// - Full English or French weekday names (`friday`, `vendredi`, ...) — the
///   NEXT future occurrence of that weekday, strictly after `today` (if
///   `today` already is that weekday, this resolves to +7 days, never +0).
pub fn resolve_relative_date(s: &str, today: chrono::NaiveDate) -> Option<chrono::NaiveDate> {
    use chrono::{Datelike, Weekday};

    let trimmed = s.trim();
    if let Ok(d) = chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        return Some(d);
    }

    let lower = trimmed.to_lowercase();
    match lower.as_str() {
        "today" | "tonight" | "aujourd'hui" | "aujourdhui" => return Some(today),
        "tomorrow" | "demain" => return Some(today + chrono::Duration::days(1)),
        _ => {}
    }

    let weekday = match lower.as_str() {
        "monday" | "lundi" => Weekday::Mon,
        "tuesday" | "mardi" => Weekday::Tue,
        "wednesday" | "mercredi" => Weekday::Wed,
        "thursday" | "jeudi" => Weekday::Thu,
        "friday" | "vendredi" => Weekday::Fri,
        "saturday" | "samedi" => Weekday::Sat,
        "sunday" | "dimanche" => Weekday::Sun,
        _ => return None,
    };

    // Next FUTURE occurrence — start the walk at tomorrow so a same-weekday
    // `today` lands 7 days out, not 0.
    let mut candidate = today + chrono::Duration::days(1);
    while candidate.weekday() != weekday {
        candidate += chrono::Duration::days(1);
    }
    Some(candidate)
}

pub fn strip_mention(text: &str) -> String {
    const MENTION: &str = "@grumps";
    let Some(pos) = find_ascii_ci(text, MENTION) else {
        return text.trim().to_string();
    };
    // `pos` and `pos + 7` are ASCII-byte boundaries (the mention is ASCII), so
    // these slices never split a codepoint.
    let before = text[..pos].trim_end();
    let after = text.get(pos + MENTION.len()..).unwrap_or("").trim_start();
    if before.is_empty() || after.is_empty() {
        format!("{before}{after}").trim().to_string()
    } else {
        format!("{before} {after}")
    }
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
    fn bare_trailing_demain() {
        let t = extract_todo_from_line("appeler le plombier demain");
        assert_eq!(t.title, "appeler le plombier");
        assert_eq!(t.deadline_text, Some("demain".into()));
    }

    #[test]
    fn deadline_avant_vendredi() {
        let t = extract_todo_from_line("pain avant vendredi");
        assert_eq!(t.title, "pain");
        assert_eq!(t.deadline_text, Some("vendredi".into()));
    }

    #[test]
    fn pour_non_date_stays_in_title() {
        let t = extract_todo_from_line("pour maman");
        assert_eq!(t.title, "pour maman");
        assert!(t.deadline_text.is_none());
    }

    #[test]
    fn le_journal_de_demain_no_deadline() {
        let t = extract_todo_from_line("le journal de demain");
        assert_eq!(t.title, "le journal de demain");
        assert!(t.deadline_text.is_none());
    }

    #[test]
    fn bare_trailing_friday() {
        let t = extract_todo_from_line("call bob friday");
        assert_eq!(t.title, "call bob");
        assert_eq!(t.deadline_text, Some("friday".into()));
    }

    // --- deadline capture abstains on trailing prose (issue #21) ---

    #[test]
    fn deadline_trigger_followed_by_prose_abstains() {
        // "sunday" looks like a date word, but "roast" trails it — the whole
        // thing must stay in the title, not become title "call bob" +
        // phantom deadline "sunday roast".
        let t = extract_todo_from_line("call bob for sunday roast");
        assert_eq!(t.title, "call bob for sunday roast");
        assert!(t.deadline_text.is_none());
    }

    #[test]
    fn deadline_trigger_fr_followed_by_prose_abstains() {
        let t = extract_todo_from_line("réserver pour vendredi soir");
        assert_eq!(t.title, "réserver pour vendredi soir");
        assert!(t.deadline_text.is_none());
    }

    #[test]
    fn deadline_trigger_terminal_still_extracts() {
        let t = extract_todo_from_line("pay rent before friday");
        assert_eq!(t.title, "pay rent");
        assert_eq!(t.deadline_text, Some("friday".into()));
    }

    #[test]
    fn deadline_trigger_followed_by_assignee_still_extracts() {
        let t = extract_todo_from_line("buy milk for tomorrow @bob");
        assert_eq!(t.title, "buy milk");
        assert_eq!(t.deadline_text, Some("tomorrow".into()));
        assert_eq!(t.assignee_mention, Some("bob".into()));
    }

    // --- strip_fast_command_mention (issue #21) ---

    #[test]
    fn fast_command_mention_leading() {
        assert_eq!(strip_fast_command_mention("@grumps quiet"), Some("quiet"));
        assert_eq!(
            strip_fast_command_mention("@heygrumpsbot quiet"),
            Some("quiet")
        );
    }

    #[test]
    fn fast_command_mention_case_insensitive() {
        assert_eq!(strip_fast_command_mention("@Grumps QUIET"), Some("QUIET"));
    }

    #[test]
    fn fast_command_mention_not_leading_returns_none() {
        assert_eq!(strip_fast_command_mention("please @grumps quiet"), None);
    }

    #[test]
    fn fast_command_mention_no_mention_returns_none() {
        assert_eq!(strip_fast_command_mention("quiet please"), None);
    }

    #[test]
    fn fast_command_mention_todo_block_returns_none() {
        assert_eq!(
            strip_fast_command_mention("TODO:\n- @grumps quiet meeting"),
            None
        );
        assert_eq!(strip_fast_command_mention("todo: @grumps quiet"), None);
        assert_eq!(strip_fast_command_mention("DONE: @grumps quiet"), None);
        assert_eq!(strip_fast_command_mention("NOTE: @grumps quiet"), None);
        assert_eq!(
            strip_fast_command_mention("NOTE [wifi]: @grumps quiet"),
            None
        );
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

    // --- resolve_relative_date ---
    // Anchor: Wednesday 2026-07-22.
    fn wed() -> chrono::NaiveDate {
        chrono::NaiveDate::from_ymd_opt(2026, 7, 22).unwrap()
    }

    #[test]
    fn resolve_passthrough_iso_date() {
        assert_eq!(
            resolve_relative_date("2026-08-01", wed()),
            chrono::NaiveDate::from_ymd_opt(2026, 8, 1)
        );
    }

    #[test]
    fn resolve_today_synonyms() {
        for s in ["today", "TODAY", "tonight", "aujourd'hui", "aujourdhui"] {
            assert_eq!(resolve_relative_date(s, wed()), Some(wed()), "input: {s}");
        }
    }

    #[test]
    fn resolve_tomorrow_synonyms() {
        let tomorrow = wed() + chrono::Duration::days(1);
        for s in ["tomorrow", "Tomorrow", "demain", "DEMAIN"] {
            assert_eq!(
                resolve_relative_date(s, wed()),
                Some(tomorrow),
                "input: {s}"
            );
        }
    }

    #[test]
    fn resolve_weekday_en_future() {
        // Wed 2026-07-22 -> next Friday is 2026-07-24.
        assert_eq!(
            resolve_relative_date("friday", wed()),
            chrono::NaiveDate::from_ymd_opt(2026, 7, 24)
        );
    }

    #[test]
    fn resolve_weekday_fr_future() {
        // Wed 2026-07-22 -> next "vendredi" is 2026-07-24.
        assert_eq!(
            resolve_relative_date("vendredi", wed()),
            chrono::NaiveDate::from_ymd_opt(2026, 7, 24)
        );
    }

    #[test]
    fn resolve_weekday_case_insensitive() {
        assert_eq!(
            resolve_relative_date("FrIdAy", wed()),
            chrono::NaiveDate::from_ymd_opt(2026, 7, 24)
        );
    }

    #[test]
    fn resolve_same_weekday_as_today_rolls_to_next_week() {
        // Today IS Wednesday: "wednesday" must resolve to +7 days, never +0.
        assert_eq!(
            resolve_relative_date("wednesday", wed()),
            Some(wed() + chrono::Duration::days(7))
        );
        assert_eq!(
            resolve_relative_date("mercredi", wed()),
            Some(wed() + chrono::Duration::days(7))
        );
    }

    #[test]
    fn resolve_all_weekdays_en() {
        let base = wed();
        let cases = [
            ("monday", 5),
            ("tuesday", 6),
            ("wednesday", 7),
            ("thursday", 1),
            ("friday", 2),
            ("saturday", 3),
            ("sunday", 4),
        ];
        for (name, days_out) in cases {
            assert_eq!(
                resolve_relative_date(name, base),
                Some(base + chrono::Duration::days(days_out)),
                "weekday: {name}"
            );
        }
    }

    #[test]
    fn resolve_garbage_returns_none() {
        for s in ["", "asap", "next month", "sometime", "🎉"] {
            assert_eq!(resolve_relative_date(s, wed()), None, "input: {s}");
        }
    }
}
