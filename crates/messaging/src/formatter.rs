use grumps_core::todo::Priority;
use crate::i18n;

/// Format a single task card (sent as individual message so users can reply to it).
pub fn task_card(
    seq_num: i64,
    title: &str,
    assignee: Option<&str>,
    deadline: Option<&str>,
    priority: Priority,
    tags: &[String],
    lang: &str,
) -> String {
    let mut lines = vec![
        format!("📋 Task #{}", seq_num),
        "━━━━━━━━━━━━━━━━━━━".into(),
        title.to_string(),
    ];
    let mut meta = Vec::new();
    if let Some(a) = assignee {
        meta.push(format!("👤 @{}", a));
    }
    if let Some(d) = deadline {
        meta.push(format!("⏰ {}", d));
    }
    if priority == Priority::High {
        meta.push(format!("🔴 {}", i18n::t("card.high_priority", lang)));
    }
    if priority == Priority::Low {
        meta.push(format!("🔵 {}", i18n::t("card.low_priority", lang)));
    }
    if !tags.is_empty() {
        meta.push(format!(
            "🏷️ {}",
            tags.iter()
                .map(|t| format!("#{}", t))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    if !meta.is_empty() {
        lines.push(String::new());
        lines.extend(meta);
    }
    lines.push(String::new());
    lines.push("━━━━━━━━━━━━━━━━━━━".into());
    lines.push(i18n::t("card.reply_hint", lang).to_string());
    lines.join("\n")
}

/// Summary after adding todos.
pub fn todos_added_summary(count: usize, workspace_slug: &str, lang: &str) -> String {
    let label = if count == 1 {
        i18n::t("todo.added", lang)
    } else {
        i18n::t("todos.added", lang)
    };
    format!(
        "✅ {} {}\n🔗 grumps.app/w/{}",
        count,
        label,
        workspace_slug
    )
}

/// Format todo list for @grumps list.
pub fn todo_list(
    todos: &[(i64, String, String, Option<String>, i32, String)],
    filter_label: &str,
    lang: &str,
) -> String {
    // Tuple: (seq_num, title, status, assignee_name, priority, tags)
    if todos.is_empty() {
        return match filter_label {
            "open" => i18n::t("empty.todos_open", lang).to_string(),
            "done" => i18n::t("empty.todos_done", lang).to_string(),
            "mine" => i18n::t("empty.todos_mine", lang).to_string(),
            _ => format!("No todos matching \"{}\".", filter_label),
        };
    }
    let mut lines = vec![
        format!(
            "📋 {} todo{} ({}):",
            todos.len(),
            if todos.len() > 1 { "s" } else { "" },
            filter_label
        ),
        String::new(),
    ];
    for (seq, title, status, assignee, priority, _tags) in todos {
        let check = if status == "done" { "✅" } else { "☐" };
        let prio = if *priority == 1 { " 🔴" } else { "" };
        let assigned = assignee
            .as_ref()
            .map(|a| format!(" → @{}", a))
            .unwrap_or_default();
        lines.push(format!("{} #{} {}{}{}", check, seq, title, assigned, prio));
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
    let mut lines = vec![
        format!("📋 Grumps Recap — {}", today),
        String::new(),
    ];

    if !high_priority.is_empty() {
        lines.push(format!("🔴 High priority ({})", high_priority.len()));
        for (seq, title, assignee, deadline) in high_priority {
            let a = assignee.as_ref().map(|a| format!(" — @{}", a)).unwrap_or_default();
            let d = deadline.as_ref().map(|d| format!(" — ⏰ {}", d)).unwrap_or_default();
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

    // 1. task_card basic (no metadata)
    #[test]
    fn test_task_card_basic() {
        let result = task_card(1, "Buy groceries", None, None, Priority::Normal, &[], "en");
        assert!(result.contains("📋 Task #1"));
        assert!(result.contains("Buy groceries"));
        assert!(result.contains("Reply: done · snooze · edit · @reassign"));
        // no metadata section
        assert!(!result.contains("👤"));
        assert!(!result.contains("⏰"));
        assert!(!result.contains("🔴"));
        assert!(!result.contains("🔵"));
    }

    // 2. task_card with all metadata
    #[test]
    fn test_task_card_all_metadata() {
        let tags = vec!["urgent".to_string(), "shopping".to_string()];
        let result = task_card(
            42,
            "Fix the thing",
            Some("Alice"),
            Some("2026-04-20"),
            Priority::High,
            &tags,
            "en",
        );
        assert!(result.contains("📋 Task #42"));
        assert!(result.contains("Fix the thing"));
        assert!(result.contains("👤 @Alice"));
        assert!(result.contains("⏰ 2026-04-20"));
        assert!(result.contains("🔴 High priority"));
        assert!(result.contains("🏷️ #urgent #shopping"));
        assert!(result.contains("Reply: done · snooze · edit · @reassign"));
    }

    // 3. task_card with high priority only
    #[test]
    fn test_task_card_high_priority_only() {
        let result = task_card(5, "Critical bug", None, None, Priority::High, &[], "en");
        assert!(result.contains("🔴 High priority"));
        assert!(!result.contains("👤"));
        assert!(!result.contains("⏰"));
        assert!(!result.contains("🔵"));
    }

    // 4. todos_added_summary singular/plural
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

    // 5. todo_list empty (open) → personality message
    #[test]
    fn test_todo_list_empty_open() {
        let result = todo_list(&[], "open", "en");
        assert_eq!(result, "Nothing to do. Suspicious.");
    }

    // 6. todo_list empty (done) → personality message
    #[test]
    fn test_todo_list_empty_done() {
        let result = todo_list(&[], "done", "en");
        assert_eq!(result, "Nothing done yet. Get to work.");
    }

    // 7. todo_list empty (mine) → personality message
    #[test]
    fn test_todo_list_empty_mine() {
        let result = todo_list(&[], "mine", "en");
        assert_eq!(result, "Nothing assigned to you. Lucky.");
    }

    // 8. todo_list with items, check format
    #[test]
    fn test_todo_list_with_items() {
        let todos = vec![
            (1i64, "Buy milk".to_string(), "open".to_string(), Some("Bob".to_string()), 2i32, String::new()),
            (2i64, "Write tests".to_string(), "open".to_string(), None, 2i32, String::new()),
        ];
        let result = todo_list(&todos, "open", "en");
        assert!(result.contains("📋 2 todos (open):"));
        assert!(result.contains("☐ #1 Buy milk → @Bob"));
        assert!(result.contains("☐ #2 Write tests"));
    }

    // 9. todo_list with done items shows ✅
    #[test]
    fn test_todo_list_done_items() {
        let todos = vec![
            (7i64, "Deploy app".to_string(), "done".to_string(), None, 1i32, String::new()),
        ];
        let result = todo_list(&todos, "done", "en");
        assert!(result.contains("✅ #7 Deploy app"));
        assert!(result.contains("🔴")); // priority 1 = high
    }

    // 10. note_list empty → personality
    #[test]
    fn test_note_list_empty() {
        let result = note_list(&[], "en");
        assert_eq!(result, "No notes. The group memory is blank.");
    }

    // 11. note_list with items
    #[test]
    fn test_note_list_with_items() {
        let notes = vec![
            ("abc123".to_string(), Some("WiFi password".to_string()), "chat".to_string(), "2026-04-01".to_string()),
            ("def456".to_string(), None, "web".to_string(), "2026-04-02".to_string()),
        ];
        let result = note_list(&notes, "en");
        assert!(result.contains("📝 2 notes:"));
        assert!(result.contains("💬 WiFi password — 2026-04-01"));
        assert!(result.contains("🌐 (untitled) — 2026-04-02"));
    }

    // 12. status_summary format
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

    // 13. help_text contains key sections
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

    // 14. recap_message with high priority items
    #[test]
    fn test_recap_with_high_priority() {
        let high = vec![
            (12i64, "Ship the project".to_string(), Some("Pierre".to_string()), Some("tomorrow".to_string())),
            (15i64, "Fix the prod bug".to_string(), Some("Sarah".to_string()), None),
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

    // 15. recap_message with no high priority items
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

    // 16. recap_message with all zeros
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
