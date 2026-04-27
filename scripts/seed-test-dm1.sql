-- Seed for the "Personal" DM test workspace D1 (test-dm1).
-- Solo workspace: 1 member (the tester), todos + notes for personal use.

DELETE FROM activity_log;
DELETE FROM notes;
DELETE FROM todos;
DELETE FROM members;

INSERT INTO members (id, platform_user_id, display_name, role, last_seen_at) VALUES
  ('m-self', '6108569905', 'Tester', 'admin', datetime('now'));

INSERT INTO todos (id, seq_num, title, status, priority, tags, created_by, source) VALUES
  ('t-dentist', 1, 'Schedule dentist appointment', 'open', 1, '["health"]', 'm-self', 'chat'),
  ('t-inbox',   2, 'Clear inbox',                  'open', 3, '["admin"]',  'm-self', 'web'),
  ('t-workout', 3, 'Workout: 3x this week',        'open', 2, '["health"]', 'm-self', 'chat');

INSERT INTO notes (id, title, content, pinned, source, created_by) VALUES
  ('n-passwords', 'Password vault', 'Manager: Bitwarden\nMaster: keep it in your head.', 1, 'web', 'm-self');

INSERT INTO activity_log (id, actor, action, target_type, target_id, source) VALUES
  ('a-dm-1', 'm-self', 'todo.create', 'todo', 't-dentist',   'chat'),
  ('a-dm-2', 'm-self', 'note.create', 'note', 'n-passwords', 'web');
