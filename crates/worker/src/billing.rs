// crates/worker/src/billing.rs
//
// Plan enum lives in `grumps_core::billing` so the agent crate (which
// can't depend on the worker) shares the same source of truth for
// agent-call and web-search quotas. The worker-specific types
// (QuotaError, check_* helpers) stay here.

pub use grumps_core::billing::Plan;

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
