//! Workspace-timezone helpers.
//!
//! Convention used across Grumps:
//! - **Instants** (reminders, scheduled actions, timed events, `created_at`,
//!   TTLs) are stored and compared in UTC — timezone-agnostic, untouched here.
//! - **Civil dates** (todo deadlines, all-day events) and **wall-clock
//!   boundaries** (now / today / this week / which weekday) are computed in the
//!   workspace timezone via these helpers, then passed to SQL as concrete bind
//!   params so SQL never calls `datetime('now')` for tz-sensitive queries.

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, TimeZone, Utc, Weekday};
use chrono_tz::Tz;

/// Parse an IANA timezone name, falling back to UTC. Single source of the
/// fallback rule (replaces the `parse().unwrap_or(chrono_tz::UTC)` idiom
/// duplicated across the agent/worker crates).
pub fn tz_or_utc(name: &str) -> Tz {
    name.parse().unwrap_or(chrono_tz::UTC)
}

/// The civil date of a UTC instant, as seen in `tz`.
pub fn date_of(instant: DateTime<Utc>, tz: Tz) -> NaiveDate {
    instant.with_timezone(&tz).date_naive()
}

/// The weekday of a UTC instant, as seen in `tz`.
pub fn weekday_of(instant: DateTime<Utc>, tz: Tz) -> Weekday {
    instant.with_timezone(&tz).weekday()
}

/// The current civil date in `tz`.
pub fn today_in_tz(tz: Tz) -> NaiveDate {
    date_of(Utc::now(), tz)
}

/// The current civil date in `tz`, formatted "YYYY-MM-DD".
pub fn tz_today_str(tz: Tz) -> String {
    today_in_tz(tz).format("%Y-%m-%d").to_string()
}

/// The current weekday in `tz`.
pub fn tz_weekday(tz: Tz) -> Weekday {
    weekday_of(Utc::now(), tz)
}

/// Resolve a naive local wall-clock datetime in `tz` to a UTC instant, handling
/// DST gap (spring-forward, time doesn't exist) and fold (fall-back, time
/// occurs twice → earliest). Mirrors `parse_user_datetime` so midnight in a DST
/// transition never panics.
pub fn local_naive_to_utc(tz: Tz, naive: NaiveDateTime) -> DateTime<Utc> {
    if let Some(dt) = tz.from_local_datetime(&naive).earliest() {
        return dt.with_timezone(&Utc);
    }
    // Gap: nudge past it (DST jumps are <= 1h in practice).
    let nudged = naive + Duration::hours(1);
    tz.from_local_datetime(&nudged)
        .earliest()
        .map(|dt| dt.with_timezone(&Utc))
        // Last resort (shouldn't happen): treat the wall clock as UTC.
        .unwrap_or_else(|| Utc.from_utc_datetime(&naive))
}

/// UTC instants bounding the tz-local calendar day: `[start_inclusive,
/// end_exclusive)` where start = local 00:00 and end = next day's local 00:00.
pub fn local_day_bounds_utc(tz: Tz, date: NaiveDate) -> (DateTime<Utc>, DateTime<Utc>) {
    let start = local_naive_to_utc(tz, date.and_hms_opt(0, 0, 0).unwrap());
    let next = date + Duration::days(1);
    let end = local_naive_to_utc(tz, next.and_hms_opt(0, 0, 0).unwrap());
    (start, end)
}

/// UTC instants bounding a tz-local window: from `from` local-00:00
/// (inclusive) to the day after `to_incl` local-00:00 (exclusive).
pub fn local_window_bounds_utc(
    tz: Tz,
    from: NaiveDate,
    to_incl: NaiveDate,
) -> (DateTime<Utc>, DateTime<Utc>) {
    let (start, _) = local_day_bounds_utc(tz, from);
    let (_, end) = local_day_bounds_utc(tz, to_incl);
    (start, end)
}

/// Format a UTC instant as RFC3339 with a trailing `Z`, second precision.
/// This is the canonical bind-param / storage shape for instants.
pub fn to_utc_z(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono_tz::Asia::Tokyo;
    use chrono_tz::Europe::Paris;

    #[test]
    fn tz_or_utc_falls_back() {
        assert_eq!(tz_or_utc("Bogus/Zone"), chrono_tz::UTC);
        assert_eq!(tz_or_utc("Europe/Paris"), Paris);
    }

    #[test]
    fn day_bounds_paris_summer() {
        // 2026-07-01 Paris is CEST (+02:00): local midnight = 22:00 prev-day UTC.
        let (start, end) =
            local_day_bounds_utc(Paris, NaiveDate::from_ymd_opt(2026, 7, 1).unwrap());
        assert_eq!(to_utc_z(start), "2026-06-30T22:00:00Z");
        assert_eq!(to_utc_z(end), "2026-07-01T22:00:00Z");
    }

    #[test]
    fn day_bounds_paris_winter() {
        // 2026-01-15 Paris is CET (+01:00): local midnight = 23:00 prev-day UTC.
        let (start, end) =
            local_day_bounds_utc(Paris, NaiveDate::from_ymd_opt(2026, 1, 15).unwrap());
        assert_eq!(to_utc_z(start), "2026-01-14T23:00:00Z");
        assert_eq!(to_utc_z(end), "2026-01-15T23:00:00Z");
    }

    #[test]
    fn day_bounds_tokyo_crosses_utc_midnight() {
        // Tokyo +09:00: local midnight = 15:00 previous-day UTC.
        let (start, _) = local_day_bounds_utc(Tokyo, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        assert_eq!(to_utc_z(start), "2025-12-31T15:00:00Z");
    }

    #[test]
    fn spring_forward_day_does_not_panic() {
        // Paris springs forward 2026-03-29. The day bounds must still resolve.
        let (start, end) =
            local_day_bounds_utc(Paris, NaiveDate::from_ymd_opt(2026, 3, 29).unwrap());
        assert!(end > start);
    }

    #[test]
    fn weekday_and_date_near_midnight_use_local_day() {
        // 2026-05-31 23:30 UTC: in Paris (CEST +02:00) it is already 2026-06-01.
        let instant = Utc.with_ymd_and_hms(2026, 5, 31, 23, 30, 0).unwrap();
        // Local civil date is the *next* day vs UTC.
        assert_ne!(date_of(instant, Paris), instant.date_naive());
        assert_eq!(
            date_of(instant, Paris),
            NaiveDate::from_ymd_opt(2026, 6, 1).unwrap()
        );
        // And the weekday differs accordingly (the case the recap gate depends on).
        assert_ne!(weekday_of(instant, Paris), instant.weekday());
    }

    #[test]
    fn window_bounds_span_local_days() {
        let (start, end) = local_window_bounds_utc(
            Paris,
            NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 7, 7).unwrap(),
        );
        assert_eq!(to_utc_z(start), "2026-06-30T22:00:00Z");
        // Exclusive end = local 00:00 of 2026-07-08.
        assert_eq!(to_utc_z(end), "2026-07-07T22:00:00Z");
    }
}
