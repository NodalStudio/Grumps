-- Seed for the local sandbox INDEX_DB. Run via:
--   npx wrangler d1 execute grumps-index --local --file=scripts/seed-local-dev.sql

-- The 3 test workspaces' D1 IDs are real CF D1s on the dev account.
-- Update these if you re-create the test DBs.

DELETE FROM user_workspaces WHERE user_id = 'user-tester';
DELETE FROM workspaces_meta WHERE slug LIKE 'test-%';
DELETE FROM user_identities WHERE user_id = 'user-tester';
DELETE FROM users WHERE id = 'user-tester';

INSERT INTO users (id, display_name, default_locale) VALUES
  ('user-tester', 'Tester', 'en');

INSERT INTO user_identities (platform, platform_user_id, user_id) VALUES
  ('telegram', '6108569905', 'user-tester');

INSERT INTO workspaces_meta (slug, platform, platform_channel_id, name, plan, d1_database_id, locale, is_dm, archived_at) VALUES
  ('test-grp1', 'telegram', '-100100100100', 'Roommates', 'free', 'd54f7a7e-f472-42f0-873d-e546cae7c4ea', 'en', 0, NULL),
  ('test-dm1',  'telegram', '6108569905',    'Personal',  'free', 'c1d24afc-18a7-4a17-9ea0-4a4924d804da', 'en', 1, NULL),
  ('test-arc1', 'telegram', '-100200200200', 'Old Group', 'free', 'd018c6cc-8fe9-4086-b92b-98fcb54704d2', 'en', 0, datetime('now'));

INSERT INTO user_workspaces (user_id, workspace_slug, role) VALUES
  ('user-tester', 'test-grp1', 'admin'),
  ('user-tester', 'test-dm1',  'admin'),
  ('user-tester', 'test-arc1', 'member');
