//! Read-only member tools: get_member_activity.
//!
//! Lets the agent judge "only if X has been silent for a while"-style
//! instructions at fire time by inspecting a member's last activity, without
//! any structured condition machinery.

use super::{args, parse_args, ToolContext};
use serde_json::{json, Value};

/// Resolve a member by name (exact display-name match, else a unique substring
/// match) and report when they were last active.
pub async fn get_member_activity(ctx: &ToolContext<'_>, raw: Value) -> worker::Result<Value> {
    let a: args::MemberActivityArgs = parse_args(raw, "get_member_activity")?;
    let needle = a.member.trim().to_lowercase();
    if needle.is_empty() {
        return Ok(json!({ "ok": false, "reason": "missing_member" }));
    }
    let members = ctx.db.list_active_members().await?;
    let name_of =
        |m: &crate::db::MemberShort| m.display_name.clone().unwrap_or_default().to_lowercase();

    // Prefer an exact display-name match; otherwise accept a unique substring.
    let exact: Vec<_> = members.iter().filter(|m| name_of(m) == needle).collect();
    let matched = match exact.as_slice() {
        [m] => Some(*m),
        _ => {
            let subs: Vec<_> = members
                .iter()
                .filter(|m| name_of(m).contains(&needle))
                .collect();
            match subs.as_slice() {
                [m] => Some(*m),
                _ => None,
            }
        }
    };

    let m = match matched {
        Some(m) => m,
        None => {
            return Ok(
                json!({ "ok": true, "found": false, "reason": "no_unique_member_match", "query": a.member }),
            )
        }
    };

    let last = ctx.db.get_member_last_seen(&m.id).await?;
    let (last_iso, seconds_since) = match last {
        Some(t) => (
            Some(t.to_rfc3339()),
            Some((chrono::Utc::now() - t).num_seconds()),
        ),
        None => (None, None),
    };
    Ok(json!({
        "ok": true,
        "found": true,
        "member": m.display_name,
        "last_active_at": last_iso,
        "seconds_since_active": seconds_since,
    }))
}
