-- Normalize legacy free-text recurrence on scheduled_actions (rows migrated from
-- the reminders table by 0005 carried strings like 'daily' / 'every monday') into
-- RRULE, so grumps_scheduler::recurrence::next_occurrence can parse them. New rows
-- are already written as RRULE by the application. Rows already in RRULE form
-- (FREQ=...) and unmappable values are left untouched.

-- Daily.
UPDATE scheduled_actions SET recurrence = 'FREQ=DAILY'
  WHERE recurrence IS NOT NULL AND recurrence NOT LIKE 'FREQ=%'
    AND (lower(recurrence) = 'daily' OR lower(recurrence) LIKE '%every day%');

-- Named weekdays.
UPDATE scheduled_actions SET recurrence = 'FREQ=WEEKLY;BYDAY=MO' WHERE recurrence IS NOT NULL AND recurrence NOT LIKE 'FREQ=%' AND lower(recurrence) LIKE '%monday%';
UPDATE scheduled_actions SET recurrence = 'FREQ=WEEKLY;BYDAY=TU' WHERE recurrence IS NOT NULL AND recurrence NOT LIKE 'FREQ=%' AND lower(recurrence) LIKE '%tuesday%';
UPDATE scheduled_actions SET recurrence = 'FREQ=WEEKLY;BYDAY=WE' WHERE recurrence IS NOT NULL AND recurrence NOT LIKE 'FREQ=%' AND lower(recurrence) LIKE '%wednesday%';
UPDATE scheduled_actions SET recurrence = 'FREQ=WEEKLY;BYDAY=TH' WHERE recurrence IS NOT NULL AND recurrence NOT LIKE 'FREQ=%' AND lower(recurrence) LIKE '%thursday%';
UPDATE scheduled_actions SET recurrence = 'FREQ=WEEKLY;BYDAY=FR' WHERE recurrence IS NOT NULL AND recurrence NOT LIKE 'FREQ=%' AND lower(recurrence) LIKE '%friday%';
UPDATE scheduled_actions SET recurrence = 'FREQ=WEEKLY;BYDAY=SA' WHERE recurrence IS NOT NULL AND recurrence NOT LIKE 'FREQ=%' AND lower(recurrence) LIKE '%saturday%';
UPDATE scheduled_actions SET recurrence = 'FREQ=WEEKLY;BYDAY=SU' WHERE recurrence IS NOT NULL AND recurrence NOT LIKE 'FREQ=%' AND lower(recurrence) LIKE '%sunday%';

-- Bare 'weekly' / 'every week' with no named day: store FREQ=WEEKLY with no
-- BYDAY. The scheduler resolves a BYDAY-less weekly rule to "same weekday as
-- the trigger, every week", computed in the workspace timezone at run time.
-- (Deriving BYDAY here via strftime('%w', trigger_at) would use the UTC
-- weekday of the stored instant, which is off by a day for reminders set near
-- local midnight in zones away from UTC.)
UPDATE scheduled_actions
SET recurrence = 'FREQ=WEEKLY'
WHERE recurrence IS NOT NULL AND recurrence NOT LIKE 'FREQ=%'
  AND (lower(recurrence) LIKE '%weekly%' OR lower(recurrence) LIKE '%every week%');
