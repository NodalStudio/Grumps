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

/// Quota-exceeded error. Returned as a typed value so callers can format
/// the user-facing message in the chat's locale instead of a hardcoded
/// English string.
#[derive(Debug, Clone, Copy)]
pub enum QuotaError {
    Todos { current: i64, max: i64 },
    Notes { current: i64, max: i64 },
    Llm { current: i64, max: i64 },
}

impl QuotaError {
    pub fn key(&self) -> &'static str {
        match self {
            Self::Todos { .. } => "billing.quota.todos",
            Self::Notes { .. } => "billing.quota.notes",
            Self::Llm   { .. } => "billing.quota.llm",
        }
    }

    pub fn current(&self) -> i64 {
        match *self { Self::Todos { current, .. } | Self::Notes { current, .. } | Self::Llm { current, .. } => current }
    }

    pub fn max(&self) -> i64 {
        match *self { Self::Todos { max, .. } | Self::Notes { max, .. } | Self::Llm { max, .. } => max }
    }

    /// Format as a localized message using the i18n catalogue.
    pub fn render(&self, locale: grumps_i18n::Locale) -> String {
        let cur = self.current().to_string();
        let mx  = self.max().to_string();
        grumps_i18n::t(locale, self.key(), &[("current", &cur), ("max", &mx)])
    }
}

/// Check if a workspace has exceeded its todo quota.
pub fn check_todo_quota(plan: &Plan, current_open: i64) -> Result<(), QuotaError> {
    if let Some(max) = plan.max_todos() {
        if current_open >= max {
            return Err(QuotaError::Todos { current: current_open, max });
        }
    }
    Ok(())
}

pub fn check_note_quota(plan: &Plan, current_count: i64) -> Result<(), QuotaError> {
    if let Some(max) = plan.max_notes() {
        if current_count >= max {
            return Err(QuotaError::Notes { current: current_count, max });
        }
    }
    Ok(())
}

pub fn check_llm_quota(plan: &Plan, calls_this_month: i64) -> Result<(), QuotaError> {
    if let Some(max) = plan.max_llm_calls() {
        if calls_this_month >= max {
            return Err(QuotaError::Llm { current: calls_this_month, max });
        }
    }
    Ok(())
}
