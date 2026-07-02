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
