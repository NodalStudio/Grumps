//! Scheduler layer for Grumps : scheduled actions, recurrence.
//! See spec § 5.3-5.4 and § 7 for schema and behavior.
//!
//! There is intentionally no structured "condition" type: conditional
//! follow-ups are expressed in natural language in a scheduled task's
//! instruction and judged by the agent at fire time (it has read-only tools —
//! `get_todo_status`, `get_member_activity` — to check live state).

pub mod action;
pub mod recurrence;
pub mod session;

pub use action::*;
pub use session::*;
