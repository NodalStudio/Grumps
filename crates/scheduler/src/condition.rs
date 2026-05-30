//! Condition evaluator for scheduled actions (conditional follow-ups).
//!
//! Conditions are an *internal* guard on a scheduled action: when present, the
//! action only fires if the condition holds at trigger time (evaluated against
//! live workspace state). They are no longer an LLM-facing DSL — the proactive
//! agent decides when to act by judgement, not by emitting a condition.
//!
//! Only conditions backed by stored state are modelled. Earlier keyword-based
//! variants (`NoMessageMatching`, `KeywordAppeared`) were removed: group message
//! bodies are not persisted, so they could never be evaluated. Such judgement
//! ("has anyone mentioned X?") is the job of the proactive path.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Duration};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Condition {
    MemberActiveAfter { member_id: String, after: DateTime<Utc> },
    TodoStatus { todo_id: String, #[serde(default)] status_not: Option<String>, #[serde(default)] status_is: Option<String> },
    MemberInactiveFor { member_id: String, duration_seconds: i64 },
}

/// Context provided by the worker when evaluating a condition.
///
/// The trait is sync; the worker pre-fetches the (small) data a condition names
/// (one todo's status / one member's last activity) via async D1 calls before
/// building a context and calling [`evaluate`].
pub trait ConditionContext {
    fn last_active_at(&self, member_id: &str) -> Option<DateTime<Utc>>;
    fn todo_status_now(&self, todo_id: &str) -> Option<String>;
    fn now(&self) -> DateTime<Utc>;
}

pub fn evaluate<C: ConditionContext>(cond: &Condition, ctx: &C) -> bool {
    match cond {
        Condition::MemberActiveAfter { member_id, after } => {
            ctx.last_active_at(member_id).map(|t| t > *after).unwrap_or(false)
        }
        Condition::TodoStatus { todo_id, status_not, status_is } => {
            let status = ctx.todo_status_now(todo_id);
            if let Some(s_not) = status_not {
                if status.as_deref() != Some(s_not.as_str()) { return true; }
            }
            if let Some(s_is) = status_is {
                if status.as_deref() == Some(s_is.as_str()) { return true; }
            }
            // If neither matched, condition is false
            status_not.is_none() && status_is.is_none()
        }
        Condition::MemberInactiveFor { member_id, duration_seconds } => {
            let now = ctx.now();
            match ctx.last_active_at(member_id) {
                None => true,                                            // never active = inactive
                Some(t) => now.signed_duration_since(t) >= Duration::seconds(*duration_seconds),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockCtx {
        last_active: Option<DateTime<Utc>>,
        todo_status: Option<String>,
        now: DateTime<Utc>,
    }
    impl ConditionContext for MockCtx {
        fn last_active_at(&self, _: &str) -> Option<DateTime<Utc>> { self.last_active }
        fn todo_status_now(&self, _: &str) -> Option<String> { self.todo_status.clone() }
        fn now(&self) -> DateTime<Utc> { self.now }
    }

    fn ctx() -> MockCtx {
        MockCtx { last_active: None, todo_status: None, now: Utc::now() }
    }

    #[test]
    fn member_active_after_fires_when_recent() {
        let cond = Condition::MemberActiveAfter {
            member_id: "m1".into(),
            after: Utc::now() - Duration::days(1),
        };
        let c = MockCtx { last_active: Some(Utc::now()), ..ctx() };
        assert!(evaluate(&cond, &c));
    }

    #[test]
    fn member_active_after_skips_when_never_seen() {
        let cond = Condition::MemberActiveAfter {
            member_id: "m1".into(),
            after: Utc::now() - Duration::days(1),
        };
        assert!(!evaluate(&cond, &ctx()));
    }

    #[test]
    fn todo_status_not_done_fires_when_status_open() {
        let cond = Condition::TodoStatus {
            todo_id: "t1".into(), status_not: Some("done".into()), status_is: None,
        };
        let c = MockCtx { todo_status: Some("open".into()), ..ctx() };
        assert!(evaluate(&cond, &c));
    }

    #[test]
    fn todo_status_not_done_skips_when_done() {
        let cond = Condition::TodoStatus {
            todo_id: "t1".into(), status_not: Some("done".into()), status_is: None,
        };
        let c = MockCtx { todo_status: Some("done".into()), ..ctx() };
        assert!(!evaluate(&cond, &c));
    }

    #[test]
    fn member_inactive_for_fires_when_long_silent() {
        let cond = Condition::MemberInactiveFor {
            member_id: "m1".into(), duration_seconds: 3600,
        };
        let c = MockCtx {
            now: Utc::now(),
            last_active: Some(Utc::now() - Duration::hours(2)),
            ..ctx()
        };
        assert!(evaluate(&cond, &c));
    }

    #[test]
    fn member_inactive_skips_when_recent() {
        let cond = Condition::MemberInactiveFor {
            member_id: "m1".into(), duration_seconds: 3600,
        };
        let c = MockCtx {
            now: Utc::now(),
            last_active: Some(Utc::now() - Duration::seconds(60)),
            ..ctx()
        };
        assert!(!evaluate(&cond, &c));
    }
}
