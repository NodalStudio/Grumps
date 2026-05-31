//! Tool registry + dispatch.

pub mod args;
pub mod calendar;
pub mod chat;
pub mod crud;
pub mod members;
pub mod memory;
pub mod rag;
pub mod rag_pipeline;
pub mod registry;
pub mod scheduler;
pub mod todos;
pub mod web;

pub use registry::{anthropic_tools, dispatch};

use crate::db::AgentDb;
use crate::router::MessagingSink;
use worker::Env;

/// Whether the agent is acting because it was addressed (`Reactive`) or on its
/// own initiative from ambient observation (`Proactive`). In `Proactive` mode a
/// mutating tool is not executed outright — it is staged for the user to confirm.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Autonomy {
    Reactive,
    Proactive,
}

/// Context passed to each tool handler.
pub struct ToolContext<'a> {
    pub env: &'a Env,
    pub workspace_slug: &'a str,
    pub member_id: &'a str,
    pub sink: &'a dyn MessagingSink,
    pub db: &'a dyn AgentDb,
    /// Resolved locale for the current interaction (member > workspace > "en").
    pub language: String,
    /// IANA timezone of the workspace (e.g. "Europe/Paris"). Used to convert
    /// the local wall-clock times the model emits into UTC for storage.
    pub timezone: String,
    /// Reactive (addressed) vs proactive (self-initiated). Gates whether mutating
    /// tools execute directly or are staged for confirmation.
    pub autonomy: Autonomy,
}

/// Parse a datetime string produced by the model into UTC.
///
/// The model is instructed to emit *local wall-clock* time in the workspace
/// timezone with no offset (e.g. "2026-05-31T20:00:00"); we interpret such a
/// naive string in `tz` and convert to UTC. If the model nonetheless includes
/// an explicit offset or a trailing `Z`, that is authoritative and respected.
/// Returns `None` if the string cannot be parsed.
pub fn parse_user_datetime(s: &str, tz: &chrono_tz::Tz) -> Option<chrono::DateTime<chrono::Utc>> {
    use chrono::TimeZone;
    let s = s.trim();
    // 1. Explicit offset / Z → authoritative.
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&chrono::Utc));
    }
    // 2. Naive wall-clock → interpret in the workspace timezone.
    const FORMATS: [&str; 4] = [
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M",
    ];
    for fmt in FORMATS {
        if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, fmt) {
            // Fold (fall-back): time occurs twice → `.single()` is None, take
            // the earliest of the two instants.
            if let Some(dt) = tz
                .from_local_datetime(&naive)
                .single()
                .or_else(|| tz.from_local_datetime(&naive).earliest())
            {
                return Some(dt.with_timezone(&chrono::Utc));
            }
            // Gap (spring-forward): the named wall-clock time doesn't exist.
            // Nudge past the gap (DST jumps are ≤ 1h in practice) so we still
            // resolve to a sensible instant rather than failing.
            let nudged = naive + chrono::Duration::hours(1);
            return tz
                .from_local_datetime(&nudged)
                .earliest()
                .map(|dt| dt.with_timezone(&chrono::Utc));
        }
    }
    None
}

// Tool dispatch + the Anthropic tool list now live in `registry` (re-exported
// above as `tools::dispatch` / `tools::anthropic_tools`). The registry is the
// single source of truth: each tool's descriptor carries its MCP annotations,
// which the proactive path consults to decide autonomous vs. propose-then-confirm.

/// Deserialize a tool's JSON arguments into its typed [`args`] struct, mapping a
/// shape mismatch to a tool-tagged error (fed back to the model as a tool_result).
pub(crate) fn parse_args<T: serde::de::DeserializeOwned>(
    args: serde_json::Value,
    tool: &str,
) -> worker::Result<T> {
    serde_json::from_value(args)
        .map_err(|e| worker::Error::RustError(format!("{tool}: invalid arguments: {e}")))
}

/// Normalize a model-emitted deadline into a **civil date** `"YYYY-MM-DD"` in
/// `tz`. A todo deadline is a calendar day, not an instant — so we never convert
/// it to UTC (which would shift the day). Accepts a bare date (kept as-is) or a
/// datetime/instant (whose date *as seen in `tz`* is taken). `None` if unparseable.
pub fn parse_user_date(s: &str, tz: &chrono_tz::Tz) -> Option<String> {
    let s = s.trim();
    if let Ok(d) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Some(d.format("%Y-%m-%d").to_string());
    }
    parse_user_datetime(s, tz).map(|utc| {
        grumps_core::timeutil::date_of(utc, *tz)
            .format("%Y-%m-%d")
            .to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_user_date, parse_user_datetime};
    use chrono_tz::America::New_York;
    use chrono_tz::Asia::{Kolkata, Tokyo};
    use chrono_tz::Europe::Paris;
    use chrono_tz::UTC;

    // ── naive local → UTC, across offsets and seasons ──────────────────────

    #[test]
    fn naive_local_interpreted_in_workspace_tz_summer() {
        // 20:00 Paris in May (CEST, +02:00) == 18:00 UTC.
        let utc = parse_user_datetime("2026-05-31T20:00:00", &Paris).unwrap();
        assert_eq!(utc.to_rfc3339(), "2026-05-31T18:00:00+00:00");
    }

    #[test]
    fn naive_local_interpreted_in_workspace_tz_winter() {
        // Same wall time in January is CET (+01:00) → 19:00 UTC. Proves the
        // offset is derived from the date, not hardcoded.
        let utc = parse_user_datetime("2026-01-15T20:00:00", &Paris).unwrap();
        assert_eq!(utc.to_rfc3339(), "2026-01-15T19:00:00+00:00");
    }

    #[test]
    fn new_york_winter_offset() {
        // 09:00 New York in January (EST, -05:00) == 14:00 UTC.
        let utc = parse_user_datetime("2026-01-15T09:00:00", &New_York).unwrap();
        assert_eq!(utc.to_rfc3339(), "2026-01-15T14:00:00+00:00");
    }

    #[test]
    fn half_hour_offset_zone() {
        // India (IST, +05:30, no DST). 10:00 local == 04:30 UTC.
        let utc = parse_user_datetime("2026-06-01T10:00:00", &Kolkata).unwrap();
        assert_eq!(utc.to_rfc3339(), "2026-06-01T04:30:00+00:00");
    }

    #[test]
    fn conversion_rolls_back_to_previous_day() {
        // 08:00 Tokyo (JST, +09:00) == 23:00 the PREVIOUS day UTC.
        let utc = parse_user_datetime("2026-01-01T08:00:00", &Tokyo).unwrap();
        assert_eq!(utc.to_rfc3339(), "2025-12-31T23:00:00+00:00");
    }

    // ── explicit offset / Z is authoritative ───────────────────────────────

    #[test]
    fn explicit_offset_is_authoritative() {
        let utc = parse_user_datetime("2026-05-31T20:00:00+02:00", &Paris).unwrap();
        assert_eq!(utc.to_rfc3339(), "2026-05-31T18:00:00+00:00");
    }

    #[test]
    fn explicit_negative_offset() {
        // Offset wins even when it disagrees with the workspace tz.
        let utc = parse_user_datetime("2026-05-31T20:00:00-04:00", &Paris).unwrap();
        assert_eq!(utc.to_rfc3339(), "2026-06-01T00:00:00+00:00");
    }

    #[test]
    fn trailing_z_is_utc_not_local() {
        let utc = parse_user_datetime("2026-05-31T20:00:00Z", &Paris).unwrap();
        assert_eq!(utc.to_rfc3339(), "2026-05-31T20:00:00+00:00");
    }

    // ── DST transitions ────────────────────────────────────────────────────

    #[test]
    fn dst_fall_back_ambiguous_picks_earliest() {
        // Paris 2026-10-25: 03:00 CEST falls back to 02:00 CET, so 02:30 occurs
        // twice. We resolve to the earliest instant (CEST, +02:00) → 00:30 UTC.
        let utc = parse_user_datetime("2026-10-25T02:30:00", &Paris).unwrap();
        assert_eq!(utc.to_rfc3339(), "2026-10-25T00:30:00+00:00");
    }

    #[test]
    fn dst_spring_forward_gap_is_nudged_not_dropped() {
        // Paris 2026-03-29: clocks jump 02:00 CET → 03:00 CEST, so 02:30 does
        // not exist. We nudge past the gap rather than failing to parse.
        let utc = parse_user_datetime("2026-03-29T02:30:00", &Paris).unwrap();
        // 03:30 CEST (+02:00) == 01:30 UTC.
        assert_eq!(utc.to_rfc3339(), "2026-03-29T01:30:00+00:00");
    }

    // ── format tolerance & failure modes ───────────────────────────────────

    #[test]
    fn accepts_space_separator_and_no_seconds() {
        let utc = parse_user_datetime("2026-05-31 20:00", &UTC).unwrap();
        assert_eq!(utc.to_rfc3339(), "2026-05-31T20:00:00+00:00");
    }

    #[test]
    fn leading_and_trailing_whitespace_trimmed() {
        let utc = parse_user_datetime("  2026-05-31T20:00:00  ", &UTC).unwrap();
        assert_eq!(utc.to_rfc3339(), "2026-05-31T20:00:00+00:00");
    }

    #[test]
    fn garbage_returns_none() {
        assert!(parse_user_datetime("not a date", &UTC).is_none());
        assert!(parse_user_datetime("", &UTC).is_none());
        assert!(parse_user_datetime("2026-13-99T99:99:99", &UTC).is_none());
    }

    #[test]
    fn user_date_keeps_bare_date_as_civil() {
        assert_eq!(
            parse_user_date("2026-06-05", &Paris).as_deref(),
            Some("2026-06-05")
        );
    }

    #[test]
    fn user_date_takes_local_date_of_a_datetime() {
        // 2026-06-05T00:30:00 local Paris is still June 5 (not shifted to June 4).
        assert_eq!(
            parse_user_date("2026-06-05T00:30:00", &Paris).as_deref(),
            Some("2026-06-05")
        );
        // A UTC instant near midnight resolves to the *local* civil date.
        // 2026-06-04T23:30:00Z is 2026-06-05 01:30 in Paris → June 5.
        assert_eq!(
            parse_user_date("2026-06-04T23:30:00Z", &Paris).as_deref(),
            Some("2026-06-05")
        );
    }

    #[test]
    fn user_date_garbage_is_none() {
        assert!(parse_user_date("someday", &UTC).is_none());
    }

    #[test]
    fn stored_format_round_trips_through_sqlite_shape() {
        // What create_reminder persists: UTC, 'Z' suffix, second precision.
        let utc = parse_user_datetime("2026-05-31T20:00:00", &Paris).unwrap();
        let stored = utc.format("%Y-%m-%dT%H:%M:%SZ").to_string();
        assert_eq!(stored, "2026-05-31T18:00:00Z");
    }

    #[test]
    fn unknown_timezone_string_falls_back_to_utc() {
        // Documents the tool-layer contract: an unparseable IANA name resolves
        // to UTC via `.parse().unwrap_or(chrono_tz::UTC)`.
        assert!("Bogus/Zone".parse::<chrono_tz::Tz>().is_err());
        let tz = "Bogus/Zone"
            .parse::<chrono_tz::Tz>()
            .unwrap_or(chrono_tz::UTC);
        let utc = parse_user_datetime("2026-05-31T20:00:00", &tz).unwrap();
        assert_eq!(utc.to_rfc3339(), "2026-05-31T20:00:00+00:00");
    }
}
