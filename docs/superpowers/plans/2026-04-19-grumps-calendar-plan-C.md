# Grumps Agent — Plan C : Calendar feature

**Goal:** Implement the calendar layer fully : aggregation view (todos+events+reminders+scheduled actions), iCal export endpoint, and (in scope) the SPA UI is in Plan F. Plan C ships the backend.

**Spec sections:** 9 (calendar — full), 12.5 (calendar UI is in Plan F).

## Tasks

### C1. todos table : add `deadline` column if missing

Check if `migrations/workspace/0001_init.sql` includes `deadline` column on `todos`. If yes, skip. If no, add migration `0006_todos_deadline.sql` :

```sql
-- Add deadline column to todos if missing
-- Idempotent : ALTER TABLE ADD COLUMN works even if exists in some SQLite versions
ALTER TABLE todos ADD COLUMN deadline TEXT;
CREATE INDEX IF NOT EXISTS idx_todos_deadline ON todos(deadline) WHERE deadline IS NOT NULL;
```

Wrap in PRAGMA / `IF NOT EXISTS`-equivalent : SQLite has no IF NOT EXISTS for ADD COLUMN. So the migration script (`migrate_workspaces.sh`) needs to handle the "duplicate column" error gracefully OR check first. Simpler : do a SELECT first in the script.

Update `scripts/migrate_workspaces.sh` to add `0006_todos_deadline` to the migration list, AND wrap with a check :
```bash
# Before applying 0006, check if column exists
HAS_DEADLINE=$(wrangler d1 execute "$db_id" --command "PRAGMA table_info(todos)" --remote --json | jq -r '.[0].results[] | select(.name=="deadline") | .name' | head -1)
if [ -z "$HAS_DEADLINE" ]; then
    wrangler d1 execute "$db_id" --file="migrations/workspace/0006_todos_deadline.sql" --remote
fi
```

Also update `crates/worker/src/provisioning.rs` to apply 0006 (no check needed — new workspaces never have it).

### C2. WorkspaceDb : list_todos_with_deadline

In `crates/worker/src/db.rs`, add :
```rust
pub async fn list_todos_with_deadline_in_range(&self, from: &str, to: &str) -> Result<Vec<TodoBrief>> { ... }
```

Returns todos where `deadline >= from AND deadline <= to AND status != 'done'`. `TodoBrief` = `{id, seq_num, title, deadline, assigned_to, priority, status}`.

Then update `agent_db_impl.rs` to wire `list_todos_with_deadline` (currently stubbed to empty Vec) to call this.

### C3. Calendar aggregator (in calendar crate)

In `crates/calendar/src/`, add module `aggregate.rs` with :
```rust
pub struct CalendarItem {
    pub id: String,                   // "todo:42" | "evt:xyz" | "rmd:..." | "sch:..."
    pub source: CalendarSource,
    pub title: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub all_day: bool,
    pub location: Option<String>,
    pub color: String,
    pub member_id: Option<String>,
    pub recurrence: Option<String>,
    pub editable: bool,
    pub url: String,
}

pub enum CalendarSource { Todo, Event, Reminder, ScheduledAction }
```

Plus `pub fn aggregate(events, todos, reminders, scheduled, from, to) -> Vec<CalendarItem>` — pure function, takes the 4 source vecs, expands recurrence (use `grumps_scheduler::recurrence`), sorts.

Test : 1 unit test with sample inputs.

### C4. REST endpoint : calendar aggregation

Update `crates/worker/src/routes/events.rs` (or new `routes/calendar.rs`) :
```
GET /api/w/:slug/calendar?from=...&to=...&members=...&sources=...
```

Calls the aggregator, returns `Vec<CalendarItem>` as JSON.

### C5. iCal token endpoints

In `crates/worker/src/routes/calendar.rs` add :
```
POST /api/w/:slug/calendar/ical-token   (admin)
DELETE /api/w/:slug/calendar/ical-token (admin)
GET /cal/:slug.ics?t=<jwt>               (public via token)
```

Token = JWT signed with `JWT_SECRET`, claims `{sub: "ical_export", ws: slug, iat}`. Stored in `settings.ical_token`. Verification : validate JWT signature THEN compare token equals `settings.ical_token` (revocation = wipe setting).

### C6. iCal generator

In `crates/calendar/src/`, add `ical.rs` :
```rust
pub fn generate_ical(workspace_name: &str, items: &[CalendarItem]) -> String
```

Builds RFC 5545 VCALENDAR with VEVENT for events, VEVENT all-day for todos with deadline, VTODO+VEVENT for completeness. RRULE included verbatim. Test : 2-3 unit tests checking VEVENT/UID/DTSTART/RRULE presence.

### C7. Wire iCal endpoint in lib.rs

Add the 3 routes above. The `/cal/:slug.ics` route is **unauthenticated** — auth is via the token query param.

### C8. Compile + test + tag

Full check, tag `plan-C-calendar`.
