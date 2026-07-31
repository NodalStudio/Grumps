-- Seed for the local sandbox INDEX_DB, wired to the D1 shim (scripts/d1-shim)
-- instead of real Cloudflare D1s. Run via:
--   npx wrangler d1 execute grumps-index --local --file=scripts/seed-e2e-index.sql
--
-- The `d1_database_id` values below are shim-only logical keys (see
-- scripts/e2e-setup.mjs, which migrates + seeds each one through the shim's
-- admin API before the worker starts) — they don't need to exist as real D1
-- databases. This is the CI/full-e2e counterpart to seed-local-dev.sql,
-- which points at real (Cloudflare-account-quota-limited) D1s for manual
-- local dev against production.

DELETE FROM user_workspaces WHERE user_id = 'user-tester';
DELETE FROM workspaces_meta WHERE slug LIKE 'test-%';
DELETE FROM user_identities WHERE user_id = 'user-tester';
DELETE FROM users WHERE id = 'user-tester';

INSERT INTO users (id, display_name, default_locale) VALUES
  ('user-tester', 'Tester', 'en');

INSERT INTO user_identities (platform, platform_user_id, user_id) VALUES
  ('telegram', '6108569905', 'user-tester');

INSERT INTO workspaces_meta (slug, platform, platform_channel_id, name, plan, d1_database_id, locale, is_dm, archived_at) VALUES
  ('test-grp1', 'telegram', '-100100100100', 'Roommates', 'free', 'e2e-test-grp1', 'en', 0, NULL),
  ('test-dm1',  'telegram', '6108569905',    'Personal',  'free', 'e2e-test-dm1',  'en', 1, NULL),
  ('test-arc1', 'telegram', '-100200200200', 'Old Group', 'free', 'e2e-test-arc1', 'en', 0, datetime('now'));

INSERT INTO user_workspaces (user_id, workspace_slug, role) VALUES
  ('user-tester', 'test-grp1', 'admin'),
  ('user-tester', 'test-dm1',  'admin'),
  ('user-tester', 'test-arc1', 'member');
