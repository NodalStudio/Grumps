// crates/worker/src/db/mod.rs
//
// Two databases live behind this module:
//
// * **Index DB** — a single global D1 (`grumps-index`) accessed through the
//   native `D1Database` binding. Holds users, sessions, the workspace
//   registry, and identity ↔ workspace mappings. Lives in `index.rs`.
//
// * **Workspace DB** — one D1 per workspace, accessed through `D1RestClient`
//   because `D1Database` bindings can only point to one statically declared
//   database. Holds chat-bound data: members, todos, notes, files,
//   scheduled actions, agent sessions, memory, activity log. Lives in
//   `workspace.rs`.
//
// The split into separate files makes the type-level boundary between
// the two DBs explicit: a workspace function can't reach for an Index-DB
// connection by accident, and vice versa.
//
// Both modules' public items are re-exported flat so existing callsites
// like `crate::db::lookup_workspace(...)` keep working unchanged.

pub mod index;
pub mod workspace;

pub use index::*;
pub use workspace::*;
