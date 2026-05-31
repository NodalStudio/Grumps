-- Drop the legacy `reminders` table.
--
-- Reminders are unified onto `scheduled_actions` (action_type = 'reminder'):
-- migration 0005 copied existing rows there, the cron-based firing path was
-- removed, and the agent's create_reminder / SetReminder path writes
-- scheduled_actions fired by the workspace Durable Object. Nothing reads or
-- writes `reminders` anymore — the SPA calendar, the agent list_calendar tool,
-- and the weekly recap now source reminders from scheduled_actions — so the
-- table and its index are removed.
DROP INDEX IF EXISTS idx_reminders_fire;
DROP TABLE IF EXISTS reminders;
