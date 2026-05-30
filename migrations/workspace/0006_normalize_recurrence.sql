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

-- Bare 'weekly' / 'every week' with no named day: derive the weekday from
-- trigger_at (strftime '%w' is 0=Sunday .. 6=Saturday).
UPDATE scheduled_actions
SET recurrence = 'FREQ=WEEKLY;BYDAY=' || (CASE strftime('%w', trigger_at)
    WHEN '0' THEN 'SU' WHEN '1' THEN 'MO' WHEN '2' THEN 'TU' WHEN '3' THEN 'WE'
    WHEN '4' THEN 'TH' WHEN '5' THEN 'FR' WHEN '6' THEN 'SA' END)
WHERE recurrence IS NOT NULL AND recurrence NOT LIKE 'FREQ=%'
  AND (lower(recurrence) LIKE '%weekly%' OR lower(recurrence) LIKE '%every week%')
  AND strftime('%w', trigger_at) IS NOT NULL;
