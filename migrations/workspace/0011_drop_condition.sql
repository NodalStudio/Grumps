-- Drop the structured `condition` column from scheduled_actions.
--
-- Conditional follow-ups are no longer modelled as a static, typed condition
-- evaluated by the runtime. Instead the "only if …" guard lives in the
-- scheduled task's natural-language instruction and is judged by the agent at
-- fire time — FollowUp/AgentTask run the full tool loop and have read-only
-- tools (get_todo_status, get_member_activity) to inspect live state. No code
-- reads or writes this column anymore, so it is removed.
ALTER TABLE scheduled_actions DROP COLUMN condition;
