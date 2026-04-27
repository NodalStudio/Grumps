//! Plan tier and per-plan quotas. Single source of truth shared between
//! the worker (storage / todo / note / LLM-call gates) and the agent
//! crate (per-month agent-call and web-search quotas).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Plan {
    Free,
    Pro,
    Business,
}

impl Plan {
    pub fn from_str(s: &str) -> Self {
        match s {
            "pro" => Self::Pro,
            "business" => Self::Business,
            _ => Self::Free,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Free => "free",
            Self::Pro => "pro",
            Self::Business => "business",
        }
    }

    /// Open-todo cap. None means unlimited.
    pub fn max_todos(&self) -> Option<i64> {
        match self {
            Self::Free => Some(25),
            Self::Pro => Some(200),
            Self::Business => None,
        }
    }

    pub fn max_notes(&self) -> Option<i64> {
        match self {
            Self::Free => Some(10),
            Self::Pro => Some(100),
            Self::Business => None,
        }
    }

    pub fn max_llm_calls(&self) -> Option<i64> {
        match self {
            Self::Free => Some(50),
            Self::Pro => Some(500),
            Self::Business => Some(2000),
        }
    }

    pub fn max_storage_bytes(&self) -> Option<i64> {
        match self {
            Self::Free => Some(100 * 1024 * 1024),
            Self::Pro => Some(5 * 1024 * 1024 * 1024),
            Self::Business => Some(50 * 1024 * 1024 * 1024),
        }
    }

    pub fn max_groups(&self) -> Option<i64> {
        match self {
            Self::Free => Some(1),
            Self::Pro => Some(5),
            Self::Business => None,
        }
    }

    /// Per-month cap on Sonnet agent-loop invocations (`run_loop` /
    /// `run_oneshot`). Counts every Sonnet call regardless of tool use.
    pub fn agent_call_quota(&self) -> u32 {
        match self {
            Self::Free => 200,
            Self::Pro => 1_000,
            Self::Business => 5_000,
        }
    }

    /// Per-month cap on `web_search` tool calls.
    pub fn web_search_quota(&self) -> u32 {
        match self {
            Self::Free => 5,
            Self::Pro => 50,
            Self::Business => 500,
        }
    }
}
