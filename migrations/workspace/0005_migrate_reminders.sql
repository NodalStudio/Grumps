-- Migrate existing reminders rows to scheduled_actions for unified handling.
-- See spec § 15.3.
-- Idempotent: NOT EXISTS check prevents double-insert.

INSERT INTO scheduled_actions (id, action_type, title, trigger_at, recurrence, payload, target_chat, created_by)
SELECT id, 'reminder', title, remind_at, recurrence,
       json_object('text', title, 'creator_member_id', created_by),
       'group', created_by
FROM reminders
WHERE status = 'active'
  AND remind_at > datetime('now')
  AND NOT EXISTS (SELECT 1 FROM scheduled_actions sa WHERE sa.id = reminders.id);

-- Keep reminders table intact for historical reference (do NOT drop).
-- The cron-based reminder handler is disabled at next deploy via wrangler.toml.
