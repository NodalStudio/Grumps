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
    // Not the `FromStr` trait: infallible (unknown input falls back to
    // `Free`), which the trait's `Result`-returning contract doesn't fit.
    #[allow(clippy::should_implement_trait)]
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

    /// How many of a `requested`-size batch of new todos fit under the plan's
    /// open-todo quota, given `current_open` already-open todos. Pure so a
    /// batch add (`TODO: a\nTODO: b\n...`) can be quota-checked per item
    /// without a DB round-trip per line — the caller inserts only the first
    /// `N` returned here and reports the quota message for the rest.
    /// `current_open` may already meet or exceed `max` (quota hit before this
    /// batch even started) — clamps to 0, never negative.
    pub fn todos_batch_allowance(&self, current_open: i64, requested: usize) -> usize {
        match self.max_todos() {
            None => requested,
            Some(max) => {
                let remaining = (max - current_open).max(0) as usize;
                remaining.min(requested)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlimited_plan_allows_the_full_batch() {
        assert_eq!(Plan::Business.todos_batch_allowance(1_000, 50), 50);
    }

    #[test]
    fn batch_fits_entirely_under_quota() {
        // Free caps at 25; 3 open + 5 requested stays under.
        assert_eq!(Plan::Free.todos_batch_allowance(3, 5), 5);
    }

    #[test]
    fn batch_is_truncated_mid_batch_when_it_crosses_the_cap() {
        // Free caps at 25; 22 open + 10 requested only has room for 3.
        assert_eq!(Plan::Free.todos_batch_allowance(22, 10), 3);
    }

    #[test]
    fn already_at_quota_allows_nothing() {
        assert_eq!(Plan::Free.todos_batch_allowance(25, 1), 0);
    }

    #[test]
    fn already_over_quota_clamps_to_zero_not_negative() {
        // Can happen if the cap was lowered after todos were created.
        assert_eq!(Plan::Free.todos_batch_allowance(30, 5), 0);
    }
}
