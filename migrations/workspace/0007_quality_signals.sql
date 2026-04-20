-- Quality signals : negative/positive feedback inferred from user messages.
-- Used to evaluate agent quality without explicit ratings.

CREATE TABLE IF NOT EXISTS quality_signals (
    id              TEXT PRIMARY KEY,
    member_id       TEXT REFERENCES members(id),
    signal_type     TEXT NOT NULL,
    -- 'silence_request' | 'forget_request' | 'correction' | 'praise' | 'confusion' | 'thanks'
    target_activity_id TEXT,              -- references activity_log.id (the bot action this signals about)
    target_activity_type TEXT,            -- denormalized from activity_log for query efficiency
    raw_text        TEXT NOT NULL,
    classifier_confidence REAL DEFAULT 0,
    classifier_reason TEXT,
    created_at      TEXT DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_quality_signals_created ON quality_signals(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_quality_signals_type ON quality_signals(signal_type, created_at DESC);
