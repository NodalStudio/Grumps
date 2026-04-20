-- Per-member locale, set automatically from the first message the
-- Gemini ambient classifier processes (it already returns a detected
-- language). Used to localise canned bot messages, recap headers,
-- inline button labels, and to inject the target locale into the agent's
-- system prompt so LLM-generated replies come back in the right language.
--
-- Default 'en' is a safe fallback — `members.locale = 'en'` means
-- "we don't know yet, treat as English".

ALTER TABLE members ADD COLUMN locale TEXT NOT NULL DEFAULT 'en';

-- Workspace-level fallback for when a member doesn't have a known locale
-- yet (first interaction). Set when the workspace is provisioned, from
-- the inviter's `Accept-Language` or the platform default.
INSERT OR IGNORE INTO settings (key, value) VALUES ('default_locale', 'en');

CREATE INDEX IF NOT EXISTS idx_members_locale ON members(locale);
