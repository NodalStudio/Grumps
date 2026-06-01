-- EMERGENCY ROLLBACK — DESTRUCTIVE. Read everything below before running.
--
-- What this script does:
--   - Rebuilds users(id, phone UNIQUE NOT NULL, created_at) by joining user_identities
--     to the current users table, filtering for platform = 'whatsapp'.
--   - Drops user_identities, sessions, users (the post-0003 shape).
--
-- What is LOST PERMANENTLY:
--   - Every user whose only identity is Telegram (or Discord) — they have NO
--     platform='whatsapp' row so they simply disappear from users. Any
--     user_workspaces rows referencing them become orphaned.
--   - Every session (all rows in sessions table).
--   - display_name and default_locale on the users table (not in the old schema).
--   - The workspaces_meta.is_dm and archived_at column VALUES (the columns
--     themselves remain — see comment below — but old code reading by name will
--     ignore them).
--
-- When to run: ONLY if 0003 was just applied, no Telegram auth has occurred,
-- and the new schema must be reverted immediately. After any Telegram login,
-- data loss from this rollback is NOT recoverable.
--
-- D1 wraps each `wrangler d1 execute --file=...` in an implicit transaction
-- and rejects explicit BEGIN/COMMIT statements.

CREATE TABLE users_old (
  id TEXT PRIMARY KEY,
  phone TEXT UNIQUE NOT NULL,
  created_at TEXT DEFAULT (datetime('now'))
);

INSERT INTO users_old (id, phone, created_at)
  SELECT ui.user_id, ui.platform_user_id, u.created_at
  FROM user_identities ui
  JOIN users u ON u.id = ui.user_id
  WHERE ui.platform = 'whatsapp';

DROP TABLE user_identities;
DROP TABLE sessions;
DROP TABLE users;
ALTER TABLE users_old RENAME TO users;

-- workspaces_meta columns are left in place (no-op for old code reading by name).
