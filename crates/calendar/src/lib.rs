//! Calendar layer for Grumps : events, aggregation, iCal export.
//! See spec § 5.2 and § 9 for schema and behavior.

pub mod event;
pub use event::*;

pub mod aggregate;
pub mod ical;
