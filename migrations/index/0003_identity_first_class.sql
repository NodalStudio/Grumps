-- Identity first-class: users.phone moves into a user_identities join table,
-- so a single user can own a Telegram account, a WhatsApp phone, and a Discord
-- ID. Also: per-device session registry for revocation + "log out everywhere",
-- and workspaces_meta gains is_dm + archived_at.
--
-- D1 wraps each `wrangler d1 execute --file=...` in an implicit transaction
-- and rejects explicit BEGIN/COMMIT statements; the statements run as one
-- atomic batch from D1's perspective.

CREATE TABLE users_new (
  id              TEXT PRIMARY KEY,
  display_name    TEXT,
  default_locale  TEXT,
  created_at      TEXT DEFAULT (datetime('now'))
);

INSERT INTO users_new (id, display_name, default_locale, created_at)
  SELECT id, NULL, NULL, created_at FROM users;

CREATE TABLE user_identities (
  platform         TEXT NOT NULL,
  platform_user_id TEXT NOT NULL,
  user_id          TEXT NOT NULL,
  verified_at      TEXT DEFAULT (datetime('now')),
  PRIMARY KEY (platform, platform_user_id)
);
CREATE INDEX idx_user_identities_user ON user_identities(user_id);

INSERT INTO user_identities (platform, platform_user_id, user_id, verified_at)
  SELECT 'whatsapp', phone, id, created_at FROM users;

DROP TABLE users;
ALTER TABLE users_new RENAME TO users;

CREATE TABLE sessions (
  id             TEXT PRIMARY KEY,
  user_id        TEXT NOT NULL,
  user_agent     TEXT,
  device_label   TEXT,
  country_hint   TEXT,
  created_at     TEXT NOT NULL DEFAULT (datetime('now')),
  last_seen_at   TEXT NOT NULL DEFAULT (datetime('now')),
  revoked_at     TEXT
);
CREATE INDEX idx_sessions_user ON sessions(user_id) WHERE revoked_at IS NULL;

ALTER TABLE workspaces_meta ADD COLUMN is_dm INTEGER NOT NULL DEFAULT 0;
ALTER TABLE workspaces_meta ADD COLUMN archived_at TEXT;
