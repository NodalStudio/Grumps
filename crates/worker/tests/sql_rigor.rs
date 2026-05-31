//! Offline SQL-semantics tests for the timezone / civil-date changes.
//!
//! D1 is SQLite, and these changes rely on standard SQLite `date()` / `datetime()`
//! semantics. We apply the real workspace migration schema to an in-memory
//! SQLite and exercise the exact query shapes the worker uses — no live D1.

use rusqlite::Connection;

fn conn() -> Connection {
    let c = Connection::open_in_memory().unwrap();
    // todos / settings live in 0001; events in 0003; scheduled_actions in 0004.
    c.execute_batch(include_str!("../../../migrations/workspace/0001_init.sql"))
        .expect("apply 0001_init");
    c.execute_batch(include_str!(
        "../../../migrations/workspace/0003_calendar.sql"
    ))
    .expect("apply 0003_calendar");
    c.execute_batch(include_str!(
        "../../../migrations/workspace/0004_scheduling.sql"
    ))
    .expect("apply 0004_scheduling");
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
fn scheduled_due_comparison_survives_t_vs_space() {
    let c = conn();
    // trigger_at is RFC3339 with 'T'/'Z'; a raw string `<=` against datetime('now')
    // (which has a space, no 'T') would break — datetime() normalizes both sides.
    // Reminders now live here as scheduled_actions (action_type='reminder').
    c.execute("INSERT INTO scheduled_actions (id, action_type, title, trigger_at, payload) VALUES ('s1', 'reminder', 'past', '2000-01-01T00:00:00Z', '{}')", []).unwrap();
    c.execute("INSERT INTO scheduled_actions (id, action_type, title, trigger_at, payload) VALUES ('s2', 'reminder', 'future', '2999-01-01T00:00:00Z', '{}')", []).unwrap();

    let due = count(&c,
        "SELECT COUNT(*) FROM scheduled_actions WHERE status='pending' AND datetime(trigger_at) <= datetime('now')",
        &[]);
    assert_eq!(due, 1, "only the past scheduled action is due");
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
    // 2026-06-03 is a Wednesday → bare "weekly" derives the day from trigger_at.
    assert_eq!(rec("c"), "FREQ=WEEKLY;BYDAY=WE");
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
