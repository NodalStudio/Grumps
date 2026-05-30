//! Agent conversation sessions.
//!
//! Multi-turn session state is persisted in the per-workspace D1
//! `agent_sessions` table and accessed through the `AgentDb` trait
//! (`get_active_agent_session` / `upsert_agent_session`), implemented by the
//! worker. There is no agent-crate-side session type yet; this module is a
//! placeholder for that logic if it ever moves out of the worker. Until then
//! the loop persists raw message history directly (see `loop_::persist_session`).
