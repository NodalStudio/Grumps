-- Calendar layer : events table
-- See spec § 5.2

CREATE TABLE IF NOT EXISTS events (
    id              TEXT PRIMARY KEY,
    title           TEXT NOT NULL,
    description     TEXT,
    starts_at       TEXT NOT NULL,
    ends_at         TEXT,
    all_day         INTEGER DEFAULT 0,
    location        TEXT,
    recurrence      TEXT,
    attendees       TEXT DEFAULT '[]',
    color           TEXT DEFAULT 'teal',
    source          TEXT DEFAULT 'web',
    related_todo_id TEXT REFERENCES todos(id),
    created_by      TEXT REFERENCES members(id),
    created_at      TEXT DEFAULT (datetime('now')),
    updated_at      TEXT DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_events_starts ON events(starts_at);
CREATE INDEX IF NOT EXISTS idx_events_recur ON events(recurrence) WHERE recurrence IS NOT NULL;
