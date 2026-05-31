//! Grumps agent : cascade router + Sonnet tool use + multi-turn sessions.
//! See spec § 8 and Plan B.

pub mod ambient;
pub mod db;
pub mod llm;
pub mod loop_;
pub mod prompt;
pub mod router;
pub mod session;
pub mod telemetry;
pub mod tools;

pub use loop_::{run_loop, run_oneshot, LoopResult};
pub use router::{route_message, MessagingSink, RouteResult};
