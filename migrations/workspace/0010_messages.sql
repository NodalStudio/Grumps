-- Chat message history log.
--
-- Why: the RAG layer embeds one Vectorize vector per message, but those vectors
-- are isolated — there is no ordered record of the conversation, so we cannot
-- pull the messages *around* a semantic match (the question and its answer often
-- live in separate messages). This table is that ordered archive.
--
-- `id` is a UUIDv7 (time-ordered) generated app-side: it is both the primary key
-- AND the ordering key, so `ORDER BY id` is chronological and neighbours are
-- fetched with `id </>= anchor LIMIT N` — no separate sequence column needed.
-- Every inbound message is stored (including very short ones) so context windows
-- are complete; the 20-char embedding threshold only gates Vectorize ingestion.
CREATE TABLE IF NOT EXISTS messages (
    id          TEXT PRIMARY KEY,             -- UUIDv7 (time-ordered)
    platform    TEXT NOT NULL,
    message_id  TEXT,                         -- platform message id (reference/dedup)
    member_id   TEXT REFERENCES members(id),
    sender_name TEXT,
    text        TEXT NOT NULL,
    created_at  TEXT DEFAULT (datetime('now'))
);
