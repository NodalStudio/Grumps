//! Grumps agent : cascade router + Sonnet tool use + multi-turn sessions.
//! See spec § 8 and Plan B.

pub mod llm;
pub mod prompt;
pub mod router;
pub mod session;
pub mod loop_;
pub mod tools;

pub use router::{route_message, RouteResult, MessagingSink};
pub use loop_::{run_loop, run_oneshot, LoopResult};
