-- Memory layer : structured workspace memory
-- See spec § 5.1

CREATE TABLE IF NOT EXISTS memory_entries (
    id              TEXT PRIMARY KEY,
    key             TEXT,
    value           TEXT NOT NULL,
    kind            TEXT NOT NULL,
    related_member  TEXT REFERENCES members(id),
    tags            TEXT DEFAULT '[]',
    source          TEXT NOT NULL,
    confidence      REAL DEFAULT 1.0,
    pinned          INTEGER DEFAULT 0,
    expires_at      TEXT,
    created_by      TEXT REFERENCES members(id),
    created_at      TEXT DEFAULT (datetime('now')),
    updated_at      TEXT DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_mem_kind ON memory_entries(kind);
CREATE INDEX IF NOT EXISTS idx_mem_pinned ON memory_entries(pinned) WHERE pinned = 1;
CREATE INDEX IF NOT EXISTS idx_mem_member ON memory_entries(related_member);
CREATE INDEX IF NOT EXISTS idx_mem_expires ON memory_entries(expires_at) WHERE expires_at IS NOT NULL;

CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
    key, value,
    content=memory_entries,
    content_rowid=rowid
);

CREATE TRIGGER IF NOT EXISTS memory_ai AFTER INSERT ON memory_entries BEGIN
    INSERT INTO memory_fts(rowid, key, value) VALUES (new.rowid, new.key, new.value);
END;

CREATE TRIGGER IF NOT EXISTS memory_ad AFTER DELETE ON memory_entries BEGIN
    INSERT INTO memory_fts(memory_fts, rowid, key, value)
        VALUES('delete', old.rowid, old.key, old.value);
END;

CREATE TRIGGER IF NOT EXISTS memory_au AFTER UPDATE ON memory_entries BEGIN
    INSERT INTO memory_fts(memory_fts, rowid, key, value)
        VALUES('delete', old.rowid, old.key, old.value);
    INSERT INTO memory_fts(rowid, key, value)
        VALUES (new.rowid, new.key, new.value);
END;
