-- Scheduling layer + agent sessions + new settings
-- See spec § 5.3-5.5

CREATE TABLE IF NOT EXISTS scheduled_actions (
    id              TEXT PRIMARY KEY,
    action_type     TEXT NOT NULL,
    title           TEXT NOT NULL,
    trigger_at      TEXT NOT NULL,
    recurrence      TEXT,
    condition       TEXT,
    payload         TEXT NOT NULL,
    target_chat     TEXT NOT NULL DEFAULT 'group',
    status          TEXT DEFAULT 'pending',
    last_fired_at   TEXT,
    last_error      TEXT,
    fire_count      INTEGER DEFAULT 0,
    created_by      TEXT REFERENCES members(id),
    created_at      TEXT DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_sched_fire ON scheduled_actions(trigger_at) WHERE status = 'pending';
CREATE INDEX IF NOT EXISTS idx_sched_status ON scheduled_actions(status);

CREATE TABLE IF NOT EXISTS agent_sessions (
    id              TEXT PRIMARY KEY,
    member_id       TEXT REFERENCES members(id),
    last_message_at TEXT NOT NULL,
    expires_at      TEXT NOT NULL,
    messages        TEXT NOT NULL,
    pending_action  TEXT,
    created_at      TEXT DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_sessions_member ON agent_sessions(member_id, expires_at);

-- New settings keys (insert if not present)
INSERT OR IGNORE INTO settings (key, value) VALUES ('proactive_mode', 'false');
INSERT OR IGNORE INTO settings (key, value) VALUES ('proactive_consent_at', '');
INSERT OR IGNORE INTO settings (key, value) VALUES ('proactive_max_per_hour', '3');
INSERT OR IGNORE INTO settings (key, value) VALUES ('auto_memory', 'false');
INSERT OR IGNORE INTO settings (key, value) VALUES ('auto_memory_consent_at', '');
INSERT OR IGNORE INTO settings (key, value) VALUES ('agent_quota_used_month', '0');
INSERT OR IGNORE INTO settings (key, value) VALUES ('web_search_quota_used_month', '0');
INSERT OR IGNORE INTO settings (key, value) VALUES ('agent_persona', 'default');
INSERT OR IGNORE INTO settings (key, value) VALUES ('ical_token', '');
