//! Offline SQL-semantics tests for the timezone / civil-date changes.
//!
//! D1 is SQLite, and these changes rely on standard SQLite `date()` / `datetime()`
//! semantics. We apply the real workspace migration schema to an in-memory
//! SQLite and exercise the exact query shapes the worker uses — no live D1.

use rusqlite::Connection;

fn conn() -> Connection {
    let c = Connection::open_in_memory().unwrap();
    // todos / reminders / settings live in 0001; events in 0003.
    c.execute_batch(include_str!("../../../migrations/workspace/0001_init.sql"))
        .expect("apply 0001_init");
    c.execute_batch(include_str!(
        "../../../migrations/workspace/0003_calendar.sql"
    ))
    .expect("apply 0003_calendar");
    c
}

fn count(c: &Connection, sql: &str, params: &[&str]) -> i64 {
    c.query_row(sql, rusqlite::params_from_iter(params.iter()), |r| r.get(0))
        .unwrap()
}

#[test]
fn todo_deadline_compared_as_civil_date() {
    let c = conn();
    // A bare civil date and a legacy instant-shaped deadline both compare by day.
    c.execute(
        "INSERT INTO todos (id, seq_num, title, deadline) VALUES ('t1', 1, 'civil', '2026-06-05')",
        [],
    )
    .unwrap();
    c.execute("INSERT INTO todos (id, seq_num, title, deadline) VALUES ('t2', 2, 'instant', '2026-06-05T09:00:00Z')", []).unwrap();

    let in_june = count(&c,
        "SELECT COUNT(*) FROM todos WHERE deadline IS NOT NULL AND date(deadline) >= date(?1) AND date(deadline) <= date(?2)",
        &["2026-06-01", "2026-06-30"]);
    assert_eq!(in_june, 2, "both shapes fall in the June day-range");

    let in_july = count(&c,
        "SELECT COUNT(*) FROM todos WHERE deadline IS NOT NULL AND date(deadline) >= date(?1) AND date(deadline) <= date(?2)",
        &["2026-07-01", "2026-07-31"]);
    assert_eq!(in_july, 0);
}

#[test]
fn event_all_day_and_timed_both_in_range() {
    let c = conn();
    c.execute(
        "INSERT INTO events (id, title, starts_at) VALUES ('e1', 'allday', '2026-05-31')",
        [],
    )
    .unwrap();
    c.execute(
        "INSERT INTO events (id, title, starts_at) VALUES ('e2', 'timed', '2026-05-31T18:00:00Z')",
        [],
    )
    .unwrap();

    let n = count(&c,
        "SELECT COUNT(*) FROM events WHERE datetime(starts_at) >= datetime(?1) AND datetime(starts_at) <= datetime(?2)",
        &["2026-05-01T00:00:00Z", "2026-06-01T00:00:00Z"]);
    assert_eq!(
        n, 2,
        "datetime() normalizes the bare date and the instant alike"
    );
}

#[test]
fn reminder_due_comparison_survives_t_vs_space() {
    let c = conn();
    // remind_at is RFC3339 with 'T'/'Z'; a raw string `<=` against datetime('now')
    // (which has a space, no 'T') would break — datetime() normalizes both sides.
    c.execute("INSERT INTO reminders (id, title, remind_at) VALUES ('r1', 'past', '2000-01-01T00:00:00Z')", []).unwrap();
    c.execute("INSERT INTO reminders (id, title, remind_at) VALUES ('r2', 'future', '2999-01-01T00:00:00Z')", []).unwrap();

    let due = count(&c,
        "SELECT COUNT(*) FROM reminders WHERE status='active' AND datetime(remind_at) <= datetime('now')",
        &[]);
    assert_eq!(due, 1, "only the past reminder is due");
}

#[test]
fn assignee_filter_is_case_insensitive() {
    // The parser lowercases the requested assignee ("@Pierre" -> "pierre") but
    // todos store the name in its original case. The list query must compare via
    // LOWER() so `list @Pierre` finds Pierre's open todos.
    let c = conn();
    c.execute(
        "INSERT INTO todos (id, seq_num, title, status, assigned_name) VALUES ('t1', 1, 'buy bread', 'open', 'Pierre')",
        [],
    )
    .unwrap();

    // The exact query shape used by get_todos_filtered's assignee branch.
    let found = count(
        &c,
        "SELECT COUNT(*) FROM todos WHERE LOWER(assigned_name) = ?1 AND status IN ('open','in_progress')",
        &["pierre"],
    );
    assert_eq!(found, 1, "lowercased filter matches original-case name");
}

#[test]
fn this_week_window_filters_by_bound() {
    let c = conn();
    c.execute("INSERT INTO todos (id, seq_num, title, status, completed_at) VALUES ('t1', 1, 'in', 'done', '2026-05-30T10:00:00Z')", []).unwrap();
    c.execute("INSERT INTO todos (id, seq_num, title, status, completed_at) VALUES ('t2', 2, 'out', 'done', '2026-05-01T10:00:00Z')", []).unwrap();

    // The worker passes a UTC week-start bound (computed from the workspace tz).
    let n = count(
        &c,
        "SELECT COUNT(*) FROM todos WHERE status='done' AND completed_at >= datetime(?1)",
        &["2026-05-25T00:00:00Z"],
    );
    assert_eq!(
        n, 1,
        "only the todo completed after the week-start bound counts"
    );
}

#[test]
fn migration_0006_normalizes_free_text_recurrence() {
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch(include_str!("../../../migrations/workspace/0001_init.sql"))
        .unwrap();
    c.execute_batch(include_str!(
        "../../../migrations/workspace/0004_scheduling.sql"
    ))
    .unwrap();
    // Free-text recurrences as carried over by migration 0005.
    c.execute("INSERT INTO scheduled_actions (id, action_type, title, trigger_at, recurrence, payload) VALUES ('a','reminder','x','2026-06-01T09:00:00Z','daily','{}')", []).unwrap();
    c.execute("INSERT INTO scheduled_actions (id, action_type, title, trigger_at, recurrence, payload) VALUES ('b','reminder','x','2026-06-01T09:00:00Z','every monday','{}')", []).unwrap();
    c.execute("INSERT INTO scheduled_actions (id, action_type, title, trigger_at, recurrence, payload) VALUES ('c','reminder','x','2026-06-03T09:00:00Z','weekly','{}')", []).unwrap();
    c.execute("INSERT INTO scheduled_actions (id, action_type, title, trigger_at, recurrence, payload) VALUES ('d','reminder','x','2026-06-01T09:00:00Z','FREQ=DAILY','{}')", []).unwrap();

    c.execute_batch(include_str!(
        "../../../migrations/workspace/0006_normalize_recurrence.sql"
    ))
    .unwrap();

    let rec = |id: &str| -> String {
        c.query_row(
            "SELECT recurrence FROM scheduled_actions WHERE id=?1",
            [id],
            |r| r.get(0),
        )
        .unwrap()
    };
    assert_eq!(rec("a"), "FREQ=DAILY");
    assert_eq!(rec("b"), "FREQ=WEEKLY;BYDAY=MO");
    // Bare "weekly" is stored without BYDAY; the scheduler resolves the weekday
    // from trigger_at in the workspace timezone at run time (deriving it here
    // via strftime would use the UTC weekday and drift a day near midnight).
    assert_eq!(rec("c"), "FREQ=WEEKLY");
    // Already an RRULE → untouched.
    assert_eq!(rec("d"), "FREQ=DAILY");
}

/// `WorkspaceDb::add_todo_tag` (issue #21: the `#tag` card reply now
/// actually persists) is a read-modify-write over the `tags` JSON-array TEXT
/// column: SELECT the current array, merge in the normalized tag in Rust,
/// UPDATE the column back. This exercises the exact two query shapes
/// against the real schema and proves round-tripping through
/// `serde_json` preserves the array shape SQLite stores as TEXT.
#[test]
fn todo_tags_read_modify_write_dedupes_and_lowercases() {
    let c = conn();
    c.execute(
        "INSERT INTO todos (id, seq_num, title, tags) VALUES ('t1', 1, 'ship it', '[\"backend\"]')",
        [],
    )
    .unwrap();

    // Mirrors `add_todo_tag`: SELECT the current tags...
    let select_tags = |c: &Connection| -> String {
        c.query_row("SELECT tags FROM todos WHERE id = ?1", ["t1"], |r| r.get(0))
            .unwrap()
    };
    let merge_and_store = |c: &Connection, new_tag: &str| {
        let current = select_tags(c);
        let normalized = new_tag.trim().to_lowercase();
        let mut tags: Vec<String> = serde_json::from_str(&current).unwrap_or_default();
        if !normalized.is_empty() && !tags.iter().any(|t| t.eq_ignore_ascii_case(&normalized)) {
            tags.push(normalized);
        }
        let tags_json = serde_json::to_string(&tags).unwrap();
        c.execute(
            "UPDATE todos SET tags = ?2, updated_at = datetime('now') WHERE id = ?1",
            rusqlite::params!["t1", tags_json],
        )
        .unwrap();
    };

    // A fresh, differently-cased tag is appended and lowercased.
    merge_and_store(&c, "Urgent");
    let tags: Vec<String> = serde_json::from_str(&select_tags(&c)).unwrap();
    assert_eq!(tags, vec!["backend".to_string(), "urgent".to_string()]);

    // Re-adding the same tag with different case is a no-op — no duplicate.
    merge_and_store(&c, "URGENT");
    let tags: Vec<String> = serde_json::from_str(&select_tags(&c)).unwrap();
    assert_eq!(
        tags,
        vec!["backend".to_string(), "urgent".to_string()],
        "case-insensitive dedupe: no duplicate 'urgent' entry"
    );
}

/// Schema slice shared by the `scheduled_actions` tests below: 0001 for
/// `members` (the `created_by` FK target), 0004 for `scheduled_actions`
/// itself.
fn conn_scheduling() -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch(include_str!("../../../migrations/workspace/0001_init.sql"))
        .expect("apply 0001_init");
    c.execute_batch(include_str!(
        "../../../migrations/workspace/0004_scheduling.sql"
    ))
    .expect("apply 0004_scheduling");
    c
}

/// `WorkspaceDb::update_scheduled_action` — mirrors the exact UPDATE shape:
/// title/trigger_at/recurrence/payload are editable, `action_type` and
/// `status` are deliberately excluded from the SET list (type is fixed at
/// creation, status is scheduler-managed). Also proves the "no row matched"
/// case the method surfaces as `Ok(false)` via `changes == 0`.
#[test]
fn update_scheduled_action_edits_only_the_editable_columns() {
    let c = conn_scheduling();
    c.execute(
        "INSERT INTO scheduled_actions (id, action_type, title, trigger_at, recurrence, payload, status) \
         VALUES ('a1', 'reminder', 'old title', '2026-06-01T09:00:00Z', 'FREQ=DAILY', '{}', 'pending')",
        [],
    )
    .unwrap();

    // Exact query shape from `WorkspaceDb::update_scheduled_action`.
    const UPDATE_SQL: &str = "UPDATE scheduled_actions SET title = ?1, trigger_at = ?2, recurrence = ?3, payload = ?4 WHERE id = ?5";
    let changes = c
        .execute(
            UPDATE_SQL,
            rusqlite::params![
                "new title",
                "2026-06-02T09:00:00Z",
                "FREQ=WEEKLY",
                "{\"text\":\"hi\"}",
                "a1"
            ],
        )
        .unwrap();
    assert_eq!(changes, 1, "one row updated maps to Ok(true)");

    let (title, trigger_at, recurrence, payload, action_type, status): (
        String,
        String,
        String,
        String,
        String,
        String,
    ) = c
        .query_row(
            "SELECT title, trigger_at, recurrence, payload, action_type, status FROM scheduled_actions WHERE id = 'a1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .unwrap();
    assert_eq!(title, "new title");
    assert_eq!(trigger_at, "2026-06-02T09:00:00Z");
    assert_eq!(recurrence, "FREQ=WEEKLY");
    assert_eq!(payload, "{\"text\":\"hi\"}");
    // Not in the SET list: untouched by the update.
    assert_eq!(action_type, "reminder", "action_type is not editable");
    assert_eq!(
        status, "pending",
        "status is scheduler-managed, not editable"
    );

    // Missing id: zero rows affected -> the method returns Ok(false).
    let changes = c
        .execute(
            UPDATE_SQL,
            rusqlite::params![
                "x",
                "2026-01-01T00:00:00Z",
                "FREQ=DAILY",
                "{}",
                "does-not-exist"
            ],
        )
        .unwrap();
    assert_eq!(changes, 0, "no matching row maps to Ok(false)");
}

/// `WorkspaceDb::set_todo_deadline` — `NULLIF(?1,'')` means passing an empty
/// string clears the deadline instead of storing an empty string, which
/// would otherwise poison every `date(deadline)` comparison used elsewhere
/// (e.g. `list_todos_with_deadline_in_range`).
#[test]
fn set_todo_deadline_empty_string_clears_via_nullif() {
    let c = conn();
    c.execute(
        "INSERT INTO todos (id, seq_num, title, deadline) VALUES ('t1', 1, 'ship it', NULL)",
        [],
    )
    .unwrap();

    // Exact query shape from `WorkspaceDb::set_todo_deadline`.
    const SET_DEADLINE_SQL: &str =
        "UPDATE todos SET deadline = NULLIF(?1,''), updated_at = datetime('now') WHERE id = ?2";

    c.execute(SET_DEADLINE_SQL, rusqlite::params!["2026-07-01", "t1"])
        .unwrap();
    let deadline: Option<String> = c
        .query_row("SELECT deadline FROM todos WHERE id = 't1'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(deadline.as_deref(), Some("2026-07-01"));

    c.execute(SET_DEADLINE_SQL, rusqlite::params!["", "t1"])
        .unwrap();
    let deadline: Option<String> = c
        .query_row("SELECT deadline FROM todos WHERE id = 't1'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        deadline, None,
        "empty string is normalized to NULL, not stored literally"
    );
}

/// `WorkspaceDb::reschedule_action_after_failure` — a transient send failure
/// on a recurring action must NOT flip it to `'failed'` (that's
/// `mark_action_failed`, a separate method for the terminal case). It
/// records the error, advances `trigger_at` past the missed occurrence, and
/// keeps `status = 'pending'` so the DO alarm keeps firing; `fire_count`
/// still increments because an attempt was made.
#[test]
fn reschedule_action_after_failure_stays_pending_and_records_error() {
    let c = conn_scheduling();
    c.execute(
        "INSERT INTO scheduled_actions (id, action_type, title, trigger_at, recurrence, payload, status, fire_count) \
         VALUES ('a1', 'reminder', 'weekly standup', '2026-06-01T09:00:00Z', 'FREQ=WEEKLY', '{}', 'pending', 3)",
        [],
    )
    .unwrap();

    // Exact query shape from `WorkspaceDb::reschedule_action_after_failure`.
    const RESCHEDULE_SQL: &str = "UPDATE scheduled_actions SET status='pending', trigger_at=?1, last_error=?2, last_fired_at=datetime('now'), fire_count=fire_count+1 WHERE id=?3";
    c.execute(
        RESCHEDULE_SQL,
        rusqlite::params!["2026-06-08T09:00:00Z", "send failed: 503", "a1"],
    )
    .unwrap();

    let (status, trigger_at, last_error, fire_count, last_fired_at): (
        String,
        String,
        Option<String>,
        i64,
        Option<String>,
    ) = c
        .query_row(
            "SELECT status, trigger_at, last_error, fire_count, last_fired_at FROM scheduled_actions WHERE id = 'a1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    assert_eq!(
        status, "pending",
        "recurring action survives a transient failure"
    );
    assert_eq!(
        trigger_at, "2026-06-08T09:00:00Z",
        "advanced past the missed occurrence"
    );
    assert_eq!(last_error.as_deref(), Some("send failed: 503"));
    assert_eq!(
        fire_count, 4,
        "fire_count still increments on a failed attempt"
    );
    assert!(last_fired_at.is_some());
}

#[test]
fn timezone_seed_default_is_utc() {
    let c = conn();
    let tz: String = c
        .query_row("SELECT value FROM settings WHERE key='timezone'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        tz, "UTC",
        "new workspaces default to UTC, not a locale-specific zone"
    );
}

/// The RAG context window (`WorkspaceDb::get_messages_around`) issues ONE bounded
/// UNION query keyed on the time-ordered UUIDv7 id. This exercises the exact
/// query shape against the real `messages` schema, and proves a SHORT reply
/// (never embedded into Vectorize) is still returned as part of the window — the
/// whole point of the messages history table.
#[test]
fn messages_window_around_anchor() {
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch(include_str!("../../../migrations/workspace/0001_init.sql"))
        .expect("apply 0001_init"); // members
    c.execute_batch(include_str!(
        "../../../migrations/workspace/0010_messages.sql"
    ))
    .expect("apply 0010_messages");

    // Lexicographically-ordered ids stand in for UUIDv7's time-ordering.
    let rows = [
        ("019e0001", "Les amis, on part ou en vacances cet ete ?"),
        ("019e0002", "L'Italie !"), // the short answer — the anchor
        ("019e0003", "Ok parfait ca me va"),
        ("019e0004", "Autre sujet sans rapport"),
    ];
    for (id, text) in rows {
        c.execute(
            "INSERT INTO messages (id, platform, text) VALUES (?1, 'telegram', ?2)",
            rusqlite::params![id, text],
        )
        .unwrap();
    }

    // The exact single-query shape used by WorkspaceDb::get_messages_around:
    // a `before` subquery (newest-first, limited), the anchor row on its own
    // (so it never consumes an `after` slot), and a strictly-after subquery,
    // re-sorted chronologically.
    const WINDOW_SQL: &str = "SELECT id, text FROM \
           (SELECT id, text FROM messages WHERE id < ?1 ORDER BY id DESC LIMIT ?2) \
         UNION ALL \
         SELECT id, text FROM messages WHERE id = ?1 \
         UNION ALL \
         SELECT id, text FROM \
           (SELECT id, text FROM messages WHERE id > ?1 ORDER BY id ASC LIMIT ?3) \
         ORDER BY id ASC";
    let anchor = "019e0002";
    let mut stmt = c.prepare(WINDOW_SQL).unwrap();
    let window: Vec<String> = stmt
        .query_map(rusqlite::params![anchor, 5_i64, 5_i64], |r| {
            r.get::<_, String>(1)
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(
        window,
        vec![
            "Les amis, on part ou en vacances cet ete ?".to_string(),
            "L'Italie !".to_string(),
            "Ok parfait ca me va".to_string(),
            "Autre sujet sans rapport".to_string(),
        ],
        "window is chronological, anchor included, and keeps the short reply"
    );
    assert!(
        window.contains(&"L'Italie !".to_string()),
        "the short (non-embedded) answer must survive in the window"
    );

    // `after` counts messages STRICTLY after the anchor: after=0 keeps the
    // anchor, and after=1 returns exactly one later message — the tool schema
    // says "how many later messages to include", so the anchor must not
    // consume a slot of that limit.
    let window0: Vec<String> = stmt
        .query_map(rusqlite::params![anchor, 1_i64, 0_i64], |r| {
            r.get::<_, String>(1)
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(
        window0,
        vec![
            "Les amis, on part ou en vacances cet ete ?".to_string(),
            "L'Italie !".to_string(),
        ],
        "after=0 still returns the anchor itself"
    );
    let window1: Vec<String> = stmt
        .query_map(rusqlite::params![anchor, 0_i64, 1_i64], |r| {
            r.get::<_, String>(1)
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(
        window1,
        vec!["L'Italie !".to_string(), "Ok parfait ca me va".to_string(),],
        "after=1 returns the anchor plus exactly one later message"
    );
}
