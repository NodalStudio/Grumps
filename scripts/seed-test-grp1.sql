-- Seed for the "Roommates" test workspace D1 (test-grp1).
-- 5 members, 6 todos with mixed priorities/statuses, 2 notes, activity log.

DELETE FROM activity_log;
DELETE FROM bot_messages;
DELETE FROM notes;
DELETE FROM todos;
DELETE FROM members;

INSERT INTO members (id, platform_user_id, display_name, role, last_seen_at) VALUES
  ('m-alice',   '6108569905', 'Tester',  'admin',  datetime('now')),
  ('m-bob',     '111111111',  'Bob',     'member', datetime('now', '-1 day')),
  ('m-charlie', '222222222',  'Charlie', 'member', datetime('now', '-3 days')),
  ('m-diana',   '333333333',  'Diana',   'member', datetime('now', '-2 hours')),
  ('m-erik',    '444444444',  'Erik',    'member', datetime('now', '-5 days'));

INSERT INTO todos (id, seq_num, title, status, priority, tags, assigned_name, created_by, source) VALUES
  ('t-buy-milk',     1, 'Buy milk',                'open',         1, '["groceries"]',    'Bob',     'm-alice', 'chat'),
  ('t-trash',        2, 'Take out the trash',      'open',         2, '["chores"]',       'Charlie', 'm-bob',   'chat'),
  ('t-rent',         3, 'Pay the rent',            'done',         1, '["bills"]',        NULL,      'm-alice', 'chat'),
  ('t-clean-kitchen',4, 'Clean the kitchen',       'open',         3, '["chores"]',       'Diana',   'm-charlie','chat'),
  ('t-buy-coffee',   5, 'Buy more coffee beans',   'open',         2, '["groceries"]',    NULL,      'm-alice', 'web'),
  ('t-call-plumber', 6, 'Call the plumber',        'in_progress',  1, '["chores"]',       'Erik',    'm-bob',   'chat');

INSERT INTO bot_messages (message_id, todo_id) VALUES
  ('msg-1', 't-buy-milk'),
  ('msg-2', 't-trash');

INSERT INTO notes (id, title, content, pinned, source, created_by) VALUES
  ('n-wifi',    'WiFi password',     'GrumpsRouter5G — wpa2 / roommates2025',                  1, 'chat', 'm-alice'),
  ('n-numbers', 'Useful numbers',    'Plumber: 06.12.34.56.78\nBuilding manager: 06.98.76.54.32', 0, 'web',  'm-bob');

INSERT INTO activity_log (id, actor, action, target_type, target_id, source) VALUES
  ('a1', 'm-alice', 'todo.create',   'todo', 't-buy-milk', 'chat'),
  ('a2', 'm-alice', 'todo.complete', 'todo', 't-rent',     'chat'),
  ('a3', 'm-bob',   'note.create',   'note', 'n-numbers',  'web');
