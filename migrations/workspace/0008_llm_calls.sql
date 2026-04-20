-- LLM call telemetry. Every Anthropic / Gemini call is logged here.
-- See observability dashboard at /w/:slug/admin/observability.

CREATE TABLE IF NOT EXISTS llm_calls (
    id              TEXT PRIMARY KEY,
    member_id       TEXT REFERENCES members(id),     -- who triggered (NULL if autonomous)
    invocation_type TEXT NOT NULL,
    -- 'classify' | 'agent_react' | 'agent_oneshot' | 'ambient' | 'proactive_filter' | 'recap'
    parent_call_id  TEXT REFERENCES llm_calls(id),   -- for trace hierarchy
    provider        TEXT NOT NULL,                    -- 'anthropic' | 'gemini'
    model           TEXT NOT NULL,
    input_tokens    INTEGER NOT NULL DEFAULT 0,
    output_tokens   INTEGER NOT NULL DEFAULT 0,
    cache_read_tokens   INTEGER DEFAULT 0,
    cache_write_tokens  INTEGER DEFAULT 0,
    cost_usd        REAL NOT NULL DEFAULT 0,
    latency_ms      INTEGER,
    tool_calls      TEXT DEFAULT '[]',               -- JSON [{name, args_size, result_size}]
    success         INTEGER NOT NULL DEFAULT 1,
    error           TEXT,
    created_at      TEXT DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_llm_calls_created ON llm_calls(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_llm_calls_type_created ON llm_calls(invocation_type, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_llm_calls_provider ON llm_calls(provider, model, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_llm_calls_parent ON llm_calls(parent_call_id) WHERE parent_call_id IS NOT NULL;
