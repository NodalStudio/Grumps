// crates/worker/src/billing.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

    pub fn as_str(&self) -> &str {
        match self { Self::Free => "free", Self::Pro => "pro", Self::Business => "business" }
    }

    pub fn max_todos(&self) -> Option<i64> {
        match self { Self::Free => Some(25), Self::Pro => Some(200), Self::Business => None }
    }

    pub fn max_notes(&self) -> Option<i64> {
        match self { Self::Free => Some(10), Self::Pro => Some(100), Self::Business => None }
    }

    pub fn max_llm_calls(&self) -> Option<i64> {
        match self { Self::Free => Some(50), Self::Pro => Some(500), Self::Business => Some(2000) }
    }

    pub fn max_storage_bytes(&self) -> Option<i64> {
        match self { Self::Free => Some(100 * 1024 * 1024), Self::Pro => Some(5 * 1024 * 1024 * 1024), Self::Business => Some(50 * 1024 * 1024 * 1024) }
    }

    pub fn max_groups(&self) -> Option<i64> {
        match self { Self::Free => Some(1), Self::Pro => Some(5), Self::Business => None }
    }
}

/// Check if a workspace has exceeded its todo quota.
pub fn check_todo_quota(plan: &Plan, current_open: i64) -> Result<(), String> {
    if let Some(max) = plan.max_todos() {
        if current_open >= max {
            return Err(format!("Todo limit reached ({}/{}). Upgrade to add more: grumps.io/billing", current_open, max));
        }
    }
    Ok(())
}

pub fn check_note_quota(plan: &Plan, current_count: i64) -> Result<(), String> {
    if let Some(max) = plan.max_notes() {
        if current_count >= max {
            return Err(format!("Note limit reached ({}/{}). Upgrade: grumps.io/billing", current_count, max));
        }
    }
    Ok(())
}

pub fn check_llm_quota(plan: &Plan, calls_this_month: i64) -> Result<(), String> {
    if let Some(max) = plan.max_llm_calls() {
        if calls_this_month >= max {
            return Err(format!("AI message limit reached ({}/{}). Upgrade: grumps.io/billing", calls_this_month, max));
        }
    }
    Ok(())
}
