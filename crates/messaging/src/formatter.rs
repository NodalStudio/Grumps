use crate::i18n;
use grumps_core::todo::Priority;

/// Literal reply hint appended to the first ~5 task cards of a workspace
/// (degressive teaching, see `card_hint_shown` in the worker). Command
/// keywords stay English in every locale — this is the stable API surface,
/// not display prose — so there is no i18n key for it.
const CARD_REPLY_HINT: &str = "↩ done · snooze · edit · @name";

/// Format a single compact task card (sent as its own message so users can
/// reply to it). Two lines by default:
///
/// ```text
/// ☐ #14 Buy milk · @bob · ⏰ Fri 26 · #groceries
/// ```
///
/// with the reply hint and/or workspace link appended as extra lines when
/// the caller asks for them (degressive hint, once-a-day link — see
/// `handler::card_chrome`).
///
/// `deadline_display` and all other content are pre-localized by the caller
/// (this crate does not depend on `grumps-i18n`, see crate-level docs);
/// the only thing this function localizes internally is nothing — it is
/// pure layout.
#[allow(clippy::too_many_arguments)]
pub fn task_card(
    seq_num: i64,
    title: &str,
    assignee: Option<&str>,
    deadline_display: Option<&str>,
    priority: Priority,
    tags: &[String],
    recurrence: bool,
    show_hint: bool,
    link_slug: Option<&str>,
) -> String {
    let mut meta = Vec::new();
    if let Some(a) = assignee {
        meta.push(format!("@{}", a));
    }
    if let Some(d) = deadline_display {
        meta.push(format!("⏰ {}", d));
    }
    match priority {
        Priority::High => meta.push("🔴".to_string()),
        Priority::Low => meta.push("🔵".to_string()),
        Priority::Normal => {}
    }
    if !tags.is_empty() {
        meta.push(
            tags.iter()
                .map(|t| format!("#{}", t))
                .collect::<Vec<_>>()
                .join(" "),
        );
    }
    if recurrence {
        meta.push("↻".to_string());
    }

    let mut line1 = format!("☐ #{} {}", seq_num, title);
    if !meta.is_empty() {
        line1.push_str(" · ");
        line1.push_str(&meta.join(" · "));
    }

    let mut lines = vec![line1];
    if show_hint {
        lines.push(CARD_REPLY_HINT.to_string());
    }
    if let Some(slug) = link_slug {
        lines.push(format!("🔗 grumps.app/w/{}", slug));
    }
    lines.join("\n")
}

/// Summary after adding todos.
pub fn todos_added_summary(count: usize, workspace_slug: &str, lang: &str) -> String {
    let label = if count == 1 {
        i18n::t("todo.added", lang)
    } else {
        i18n::t("todos.added", lang)
    };
    format!("✅ {} {}\n🔗 grumps.app/w/{}", count, label, workspace_slug)
}

/// One row for [`todo_list`], with locale-aware display fields precomputed by
/// the caller (deadline localization, sort bucket) so this crate stays free
/// of any `grumps-i18n` dependency — see crate-level docs. `sort_key` is
/// `(group, date_ordinal, priority_rank, seq_num)`:
/// - `group`: 0 = overdue, 1 = today, 2 = dated (future), 3 = undated.
/// - `date_ordinal`: proleptic-Gregorian ordinal day for group 2, else 0.
/// - `priority_rank`: 0 = High, 1 = Normal, 2 = Low.
/// - `seq_num`: final tiebreaker, ascending.
pub struct TodoListItem {
    pub seq_num: i64,
    pub title: String,
    pub done: bool,
    pub assignee: Option<String>,
    pub priority: Priority,
    pub tags: Vec<String>,
    pub deadline_display: Option<String>,
    pub sort_key: (u8, i64, u8, i64),
}

/// Format the todo list for `@grumps list` — grouped (overdue → today →
/// dated ascending → undated) and using the same compact visual atoms as
/// [`task_card`]. `header` is the pre-rendered, pluralized count phrase
/// (e.g. `"3 open todos"`, already localized by the caller via `t_plural`);
/// `filter_label` is only consulted for the empty-state personality message,
/// unchanged from the previous format.
pub fn todo_list(items: &[TodoListItem], filter_label: &str, header: &str, lang: &str) -> String {
    if items.is_empty() {
        return match filter_label {
            "open" => i18n::t("empty.todos_open", lang).to_string(),
            "done" => i18n::t("empty.todos_done", lang).to_string(),
            "mine" => i18n::t("empty.todos_mine", lang).to_string(),
            _ => format!("No todos matching \"{}\".", filter_label),
        };
    }

    let mut sorted: Vec<&TodoListItem> = items.iter().collect();
    sorted.sort_by_key(|i| i.sort_key);

    let mut lines = vec![format!("📋 {}", header)];
    for item in sorted {
        let check = if item.done { "✅" } else { "☐" };
        let mut line = format!("{} #{} {}", check, item.seq_num, item.title);

        let mut meta = Vec::new();
        if let Some(d) = &item.deadline_display {
            meta.push(d.clone());
        }
        if let Some(a) = &item.assignee {
            meta.push(format!("@{}", a));
        }
        if item.priority == Priority::High {
            meta.push("🔴".to_string());
        }
        if !item.tags.is_empty() {
            meta.push(
                item.tags
                    .iter()
                    .map(|t| format!("#{}", t))
                    .collect::<Vec<_>>()
                    .join(" "),
            );
        }
        if !meta.is_empty() {
            line.push_str(" · ");
            line.push_str(&meta.join(" · "));
        }
        lines.push(line);
    }
    lines.join("\n")
}

/// Format note list.
pub fn note_list(notes: &[(String, Option<String>, String, String)], lang: &str) -> String {
    // Tuple: (id, title, source, created_at)
    if notes.is_empty() {
        return i18n::t("empty.notes", lang).to_string();
    }
    let mut lines = vec![
        format!(
            "📝 {} note{}:",
            notes.len(),
            if notes.len() > 1 { "s" } else { "" }
        ),
        String::new(),
    ];
    for (_id, title, source, created_at) in notes {
        let t = title.as_deref().unwrap_or("(untitled)");
        let badge = if source == "chat" { "💬" } else { "🌐" };
        lines.push(format!("• {} {} — {}", badge, t, created_at));
    }
    lines.join("\n")
}

/// Status summary for @grumps (no args).
pub fn status_summary(
    open_todos: i64,
    done_week: i64,
    notes: i64,
    files: i64,
    workspace_slug: &str,
    _lang: &str,
) -> String {
    vec![
        "📊 Status".into(),
        "━━━━━━━━━━━━━━━━━━━".into(),
        format!("☐ {} open", open_todos),
        format!("✅ {} done this week", done_week),
        format!("📝 {} notes", notes),
        format!("📎 {} files", files),
        String::new(),
        format!("🔗 grumps.app/w/{}", workspace_slug),
    ]
    .join("\n")
}

/// Format a weekly/daily recap message.
pub fn recap_message(
    slug: &str,
    open: i64,
    assigned: i64,
    done_week: i64,
    high_priority: &[(i64, String, Option<String>, Option<String>)],
    new_notes: i64,
    reminders: i64,
    _lang: &str,
) -> String {
    use chrono::Utc;
    let today = Utc::now().format("%A %B %-d").to_string();
    let mut lines = vec![format!("📋 Grumps Recap — {}", today), String::new()];

    if !high_priority.is_empty() {
        lines.push(format!("🔴 High priority ({})", high_priority.len()));
        for (seq, title, assignee, deadline) in high_priority {
            let a = assignee
                .as_ref()
                .map(|a| format!(" — @{}", a))
                .unwrap_or_default();
            let d = deadline
                .as_ref()
                .map(|d| format!(" — ⏰ {}", d))
                .unwrap_or_default();
            lines.push(format!("  • #{} {}{}{}", seq, title, a, d));
        }
        lines.push(String::new());
    }

    lines.push(format!("📌 Open todos: {} ({} assigned)", open, assigned));
    lines.push(format!("✅ Completed this week: {}", done_week));
    lines.push(format!("📝 New notes: {}", new_notes));
    lines.push(format!("⏰ Upcoming reminders: {}", reminders));
    lines.push(String::new());
    lines.push(format!("🔗 grumps.app/w/{}", slug));

    lines.join("\n")
}

/// Help text.
pub fn help_text() -> String {
    vec![
        "📋 *Grumps* — Gets it done.",
        "",
        "*Add todos:*",
        "  TODO:",
        "  • Item one",
        "  • Item two @person !high #tag",
        "  _or_ @grumps buy bread @Bob",
        "",
        "*Complete:*",
        "  DONE: • bread",
        "  _or_ reply \"done\" to a task card",
        "  _or_ @grumps done #42",
        "",
        "*List:*",
        "  @grumps list",
        "  @grumps list all / mine / done",
        "  @grumps list @person / #tag",
        "",
        "*Notes:*",
        "  NOTE: wifi password is XYZ",
        "  @grumps notes / @grumps note wifi",
        "",
        "*Reply + @grumps:*",
        "  Reply to any msg + @grumps todo",
        "  Reply to any msg + @grumps note",
        "",
        "*Other:*",
        "  @grumps delete #42",
        "  @grumps link / help",
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- task_card ---------------------------------------------------------

    // 1. Minimal card: title only, no meta, no hint, no link.
    #[test]
    fn test_task_card_minimal() {
        let result = task_card(
            1,
            "Buy groceries",
            None,
            None,
            Priority::Normal,
            &[],
            false,
            false,
            None,
        );
        assert_eq!(result, "☐ #1 Buy groceries");
    }

    // 2. Full card: every field populated.
    #[test]
    fn test_task_card_full() {
        let tags = vec!["urgent".to_string(), "shopping".to_string()];
        let result = task_card(
            42,
            "Fix the thing",
            Some("Alice"),
            Some("Fri 26"),
            Priority::High,
            &tags,
            true,
            true,
            Some("6f4ac5e1"),
        );
        let expected = "☐ #42 Fix the thing · @Alice · ⏰ Fri 26 · 🔴 · #urgent #shopping · ↻\n\
             ↩ done · snooze · edit · @name\n\
             🔗 grumps.app/w/6f4ac5e1";
        assert_eq!(result, expected);
    }

    // 3. High priority only shows 🔴, no label text.
    #[test]
    fn test_task_card_high_priority_only() {
        let result = task_card(
            5,
            "Critical bug",
            None,
            None,
            Priority::High,
            &[],
            false,
            false,
            None,
        );
        assert_eq!(result, "☐ #5 Critical bug · 🔴");
    }

    // 4. Low priority shows 🔵.
    #[test]
    fn test_task_card_low_priority() {
        let result = task_card(
            6,
            "Someday maybe",
            None,
            None,
            Priority::Low,
            &[],
            false,
            false,
            None,
        );
        assert_eq!(result, "☐ #6 Someday maybe · 🔵");
    }

    // 5. Normal priority adds no marker at all.
    #[test]
    fn test_task_card_normal_priority_silent() {
        let result = task_card(
            7,
            "Normal thing",
            None,
            None,
            Priority::Normal,
            &[],
            false,
            false,
            None,
        );
        assert!(!result.contains('🔴'));
        assert!(!result.contains('🔵'));
    }

    // 6. Hint line toggled on.
    #[test]
    fn test_task_card_hint_on() {
        let result = task_card(1, "X", None, None, Priority::Normal, &[], false, true, None);
        assert_eq!(result, "☐ #1 X\n↩ done · snooze · edit · @name");
    }

    // 7. Hint line toggled off (default).
    #[test]
    fn test_task_card_hint_off() {
        let result = task_card(
            1,
            "X",
            None,
            None,
            Priority::Normal,
            &[],
            false,
            false,
            None,
        );
        assert!(!result.contains("↩"));
    }

    // 8. Link line toggled on.
    #[test]
    fn test_task_card_link_on() {
        let result = task_card(
            1,
            "X",
            None,
            None,
            Priority::Normal,
            &[],
            false,
            false,
            Some("my-ws"),
        );
        assert_eq!(result, "☐ #1 X\n🔗 grumps.app/w/my-ws");
    }

    // 9. Link line toggled off (default).
    #[test]
    fn test_task_card_link_off() {
        let result = task_card(
            1,
            "X",
            None,
            None,
            Priority::Normal,
            &[],
            false,
            false,
            None,
        );
        assert!(!result.contains("🔗"));
    }

    // 10. Recurrence marker.
    #[test]
    fn test_task_card_recurrence_marker() {
        let result = task_card(
            1,
            "Water plants",
            None,
            None,
            Priority::Normal,
            &[],
            true,
            false,
            None,
        );
        assert_eq!(result, "☐ #1 Water plants · ↻");
    }

    // 11. No recurrence marker by default.
    #[test]
    fn test_task_card_no_recurrence_marker() {
        let result = task_card(
            1,
            "Water plants",
            None,
            None,
            Priority::Normal,
            &[],
            false,
            false,
            None,
        );
        assert!(!result.contains('↻'));
    }

    // --- todos_added_summary -------------------------------------------------

    #[test]
    fn test_todos_added_summary_singular() {
        let result = todos_added_summary(1, "my-workspace", "en");
        assert_eq!(result, "✅ 1 todo added\n🔗 grumps.app/w/my-workspace");
    }

    #[test]
    fn test_todos_added_summary_plural() {
        let result = todos_added_summary(3, "team-alpha", "en");
        assert_eq!(result, "✅ 3 todos added\n🔗 grumps.app/w/team-alpha");
    }

    // --- todo_list -------------------------------------------------------------

    fn item(
        seq: i64,
        title: &str,
        done: bool,
        assignee: Option<&str>,
        priority: Priority,
        tags: &[&str],
        deadline_display: Option<&str>,
        sort_key: (u8, i64, u8, i64),
    ) -> TodoListItem {
        TodoListItem {
            seq_num: seq,
            title: title.to_string(),
            done,
            assignee: assignee.map(str::to_string),
            priority,
            tags: tags.iter().map(|t| t.to_string()).collect(),
            deadline_display: deadline_display.map(str::to_string),
            sort_key,
        }
    }

    // 12. todo_list empty (open) → personality message, unchanged.
    #[test]
    fn test_todo_list_empty_open() {
        let result = todo_list(&[], "open", "3 open todos", "en");
        assert_eq!(result, "Nothing to do. Suspicious.");
    }

    // 13. todo_list empty (done) → personality message, unchanged.
    #[test]
    fn test_todo_list_empty_done() {
        let result = todo_list(&[], "done", "", "en");
        assert_eq!(result, "Nothing done yet. Get to work.");
    }

    // 14. todo_list empty (mine) → personality message, unchanged.
    #[test]
    fn test_todo_list_empty_mine() {
        let result = todo_list(&[], "mine", "", "en");
        assert_eq!(result, "Nothing assigned to you. Lucky.");
    }

    // 15. Header uses the pre-rendered, localized phrase verbatim.
    #[test]
    fn test_todo_list_header() {
        let items = vec![item(
            1,
            "Buy milk",
            false,
            None,
            Priority::Normal,
            &[],
            None,
            (3, 0, 1, 1),
        )];
        let result = todo_list(&items, "open", "1 open todo", "en");
        assert!(result.starts_with("📋 1 open todo\n"));
    }

    // 16. Done checkmark vs open checkbox.
    #[test]
    fn test_todo_list_done_checkmark() {
        let items = vec![
            item(
                1,
                "Open one",
                false,
                None,
                Priority::Normal,
                &[],
                None,
                (3, 0, 1, 1),
            ),
            item(
                2,
                "Done one",
                true,
                None,
                Priority::Normal,
                &[],
                None,
                (3, 0, 1, 2),
            ),
        ];
        let result = todo_list(&items, "all", "2 todos", "en");
        assert!(result.contains("☐ #1 Open one"));
        assert!(result.contains("✅ #2 Done one"));
    }

    // 17. Item meta order: deadline, assignee, priority, tags.
    #[test]
    fn test_todo_list_item_full_meta() {
        let items = vec![item(
            12,
            "Relancer le proprio",
            false,
            Some("bob"),
            Priority::High,
            &["urgent"],
            Some("⚠ hier"),
            (0, 0, 0, 12),
        )];
        let result = todo_list(&items, "open", "1 open todo", "en");
        assert!(result.contains("☐ #12 Relancer le proprio · ⚠ hier · @bob · 🔴 · #urgent"));
    }

    // 18. Grouping/sort order: overdue, today, dated ascending, undated —
    // and within a group, high priority before seq order.
    #[test]
    fn test_todo_list_grouping_order() {
        let items = vec![
            item(
                9,
                "Undated low prio",
                false,
                None,
                Priority::Low,
                &[],
                None,
                (3, 0, 2, 9),
            ),
            item(
                14,
                "Dated later",
                false,
                None,
                Priority::Normal,
                &[],
                Some("Sat 27"),
                (2, 20000, 1, 14),
            ),
            item(
                3,
                "Undated high prio",
                false,
                None,
                Priority::High,
                &[],
                None,
                (3, 0, 0, 3),
            ),
            item(
                12,
                "Overdue",
                false,
                None,
                Priority::Normal,
                &[],
                Some("⚠ Thu 24"),
                (0, 0, 1, 12),
            ),
            item(
                5,
                "Today",
                false,
                None,
                Priority::Normal,
                &[],
                Some("today"),
                (1, 0, 1, 5),
            ),
            item(
                1,
                "Dated sooner",
                false,
                None,
                Priority::Normal,
                &[],
                Some("Fri 26"),
                (2, 19000, 1, 1),
            ),
        ];
        let result = todo_list(&items, "all", "6 todos", "en");
        let order: Vec<i64> = result
            .lines()
            .skip(1)
            .map(|l| {
                l.split('#')
                    .nth(1)
                    .unwrap()
                    .split_whitespace()
                    .next()
                    .unwrap()
                    .parse()
                    .unwrap()
            })
            .collect();
        // overdue(12) -> today(5) -> dated asc(1, 14) -> undated: high(3) first, then seq(9)
        assert_eq!(order, vec![12, 5, 1, 14, 3, 9]);
    }

    // --- note_list / status_summary / recap_message (unchanged) ---------------

    #[test]
    fn test_note_list_empty() {
        let result = note_list(&[], "en");
        assert_eq!(result, "No notes. The group memory is blank.");
    }

    #[test]
    fn test_note_list_with_items() {
        let notes = vec![
            (
                "abc123".to_string(),
                Some("WiFi password".to_string()),
                "chat".to_string(),
                "2026-04-01".to_string(),
            ),
            (
                "def456".to_string(),
                None,
                "web".to_string(),
                "2026-04-02".to_string(),
            ),
        ];
        let result = note_list(&notes, "en");
        assert!(result.contains("📝 2 notes:"));
        assert!(result.contains("💬 WiFi password — 2026-04-01"));
        assert!(result.contains("🌐 (untitled) — 2026-04-02"));
    }

    #[test]
    fn test_status_summary() {
        let result = status_summary(5, 3, 12, 7, "my-team", "en");
        assert!(result.contains("📊 Status"));
        assert!(result.contains("☐ 5 open"));
        assert!(result.contains("✅ 3 done this week"));
        assert!(result.contains("📝 12 notes"));
        assert!(result.contains("📎 7 files"));
        assert!(result.contains("🔗 grumps.app/w/my-team"));
    }

    #[test]
    fn test_help_text_key_sections() {
        let result = help_text();
        assert!(result.contains("*Grumps*"));
        assert!(result.contains("*Add todos:*"));
        assert!(result.contains("*Complete:*"));
        assert!(result.contains("*List:*"));
        assert!(result.contains("*Notes:*"));
        assert!(result.contains("*Reply + @grumps:*"));
        assert!(result.contains("*Other:*"));
        assert!(result.contains("@grumps list"));
        assert!(result.contains("@grumps done #42"));
    }

    #[test]
    fn test_recap_with_high_priority() {
        let high = vec![
            (
                12i64,
                "Ship the project".to_string(),
                Some("Pierre".to_string()),
                Some("tomorrow".to_string()),
            ),
            (
                15i64,
                "Fix the prod bug".to_string(),
                Some("Sarah".to_string()),
                None,
            ),
        ];
        let result = recap_message("x7k9m2p4", 7, 3, 5, &high, 2, 1, "en");
        assert!(result.contains("📋 Grumps Recap —"));
        assert!(result.contains("🔴 High priority (2)"));
        assert!(result.contains("  • #12 Ship the project — @Pierre — ⏰ tomorrow"));
        assert!(result.contains("  • #15 Fix the prod bug — @Sarah"));
        assert!(result.contains("📌 Open todos: 7 (3 assigned)"));
        assert!(result.contains("✅ Completed this week: 5"));
        assert!(result.contains("📝 New notes: 2"));
        assert!(result.contains("⏰ Upcoming reminders: 1"));
        assert!(result.contains("🔗 grumps.app/w/x7k9m2p4"));
    }

    #[test]
    fn test_recap_no_high_priority() {
        let result = recap_message("abc123", 3, 1, 2, &[], 0, 0, "en");
        assert!(!result.contains("🔴 High priority"));
        assert!(result.contains("📌 Open todos: 3 (1 assigned)"));
        assert!(result.contains("✅ Completed this week: 2"));
        assert!(result.contains("📝 New notes: 0"));
        assert!(result.contains("⏰ Upcoming reminders: 0"));
        assert!(result.contains("🔗 grumps.app/w/abc123"));
    }

    #[test]
    fn test_recap_all_zeros() {
        let result = recap_message("empty-ws", 0, 0, 0, &[], 0, 0, "en");
        assert!(result.contains("📋 Grumps Recap —"));
        assert!(!result.contains("🔴 High priority"));
        assert!(result.contains("📌 Open todos: 0 (0 assigned)"));
        assert!(result.contains("✅ Completed this week: 0"));
        assert!(result.contains("📝 New notes: 0"));
        assert!(result.contains("⏰ Upcoming reminders: 0"));
        assert!(result.contains("🔗 grumps.app/w/empty-ws"));
    }
}
