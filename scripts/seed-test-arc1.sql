-- Seed for the archived test workspace D1 (test-arc1).
-- Used to verify that archived workspaces still load read-only data.

DELETE FROM activity_log;
DELETE FROM notes;
DELETE FROM todos;
DELETE FROM members;

INSERT INTO members (id, platform_user_id, display_name, role, last_seen_at) VALUES
  ('m-old-self', '6108569905', 'Tester', 'member', datetime('now', '-30 days'));

INSERT INTO todos (id, seq_num, title, status, priority, tags, created_by, source) VALUES
  ('t-archived', 1, 'Old todo from a dead group', 'done', 2, '[]', 'm-old-self', 'chat');
