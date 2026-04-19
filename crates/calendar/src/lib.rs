//! Calendar layer for Grumps : events, aggregation, iCal export.
//! Plan A scope : event types only.
//! See spec § 5.2 and § 9 for schema and behavior.

pub mod event;
pub use event::*;
