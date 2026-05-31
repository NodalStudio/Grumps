//! Minimal RRULE expander supporting the 5 launch-time cases :
//! - FREQ=DAILY
//! - FREQ=WEEKLY;BYDAY=...
//! - FREQ=WEEKLY;BYDAY=...;INTERVAL=N
//! - FREQ=MONTHLY;BYMONTHDAY=N
//! - FREQ=YEARLY;BYMONTH=M;BYMONTHDAY=N
//!
//! See spec § 7.6.

use chrono::{
    DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Timelike, Utc, Weekday,
};
use chrono_tz::Tz;
use grumps_core::timeutil::local_naive_to_utc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RruleError {
    #[error("Unsupported FREQ: {0}")]
    UnsupportedFreq(String),
    #[error("Invalid RRULE syntax: {0}")]
    InvalidSyntax(String),
    #[error("Missing required field: {0}")]
    MissingField(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rrule {
    pub freq: Freq,
    pub interval: u32,             // default 1
    pub by_day: Vec<Weekday>,      // for WEEKLY
    pub by_month_day: Option<u32>, // for MONTHLY/YEARLY
    pub by_month: Option<u32>,     // for YEARLY
    pub by_hour: Option<u32>,      // optional for any FREQ
    pub by_minute: Option<u32>,    // optional for any FREQ
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Freq {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

pub fn parse(rrule: &str) -> Result<Rrule, RruleError> {
    let mut freq: Option<Freq> = None;
    let mut interval = 1u32;
    let mut by_day = vec![];
    let mut by_month_day = None;
    let mut by_month = None;
    let mut by_hour = None;
    let mut by_minute = None;

    for part in rrule.split(';') {
        let mut kv = part.splitn(2, '=');
        let k = kv
            .next()
            .ok_or_else(|| RruleError::InvalidSyntax(rrule.into()))?;
        let v = kv
            .next()
            .ok_or_else(|| RruleError::InvalidSyntax(rrule.into()))?;
        match k {
            "FREQ" => {
                freq = Some(match v {
                    "DAILY" => Freq::Daily,
                    "WEEKLY" => Freq::Weekly,
                    "MONTHLY" => Freq::Monthly,
                    "YEARLY" => Freq::Yearly,
                    other => return Err(RruleError::UnsupportedFreq(other.into())),
                })
            }
            "INTERVAL" => interval = v.parse().map_err(|_| RruleError::InvalidSyntax(v.into()))?,
            "BYDAY" => {
                by_day = v
                    .split(',')
                    .map(parse_weekday)
                    .collect::<Result<Vec<_>, _>>()?
            }
            "BYMONTHDAY" => {
                by_month_day = Some(v.parse().map_err(|_| RruleError::InvalidSyntax(v.into()))?)
            }
            "BYMONTH" => {
                by_month = Some(v.parse().map_err(|_| RruleError::InvalidSyntax(v.into()))?)
            }
            "BYHOUR" => by_hour = Some(v.parse().map_err(|_| RruleError::InvalidSyntax(v.into()))?),
            "BYMINUTE" => {
                by_minute = Some(v.parse().map_err(|_| RruleError::InvalidSyntax(v.into()))?)
            }
            _ => { /* ignore unknown */ }
        }
    }

    Ok(Rrule {
        freq: freq.ok_or_else(|| RruleError::MissingField("FREQ".into()))?,
        interval,
        by_day,
        by_month_day,
        by_month,
        by_hour,
        by_minute,
    })
}

/// Convert a free-text recurrence (the legacy reminders format, e.g. "daily",
/// "every monday", "weekly") into an RRULE string the scheduler understands.
/// `weekday` is the trigger's local weekday, used when a bare "weekly" has no
/// named day. Returns `None` for one-off / unrecognized input (fires once).
/// An input that is already an RRULE (`FREQ=...`) is passed through.
pub fn text_to_rrule(s: &str, weekday: Weekday) -> Option<String> {
    let t = s.trim().to_lowercase();
    if t.is_empty() {
        return None;
    }
    if t.starts_with("freq=") {
        return Some(s.trim().to_uppercase());
    }
    if t.contains("every day") || t == "daily" {
        return Some("FREQ=DAILY".into());
    }
    for (name, code) in [
        ("monday", "MO"),
        ("tuesday", "TU"),
        ("wednesday", "WE"),
        ("thursday", "TH"),
        ("friday", "FR"),
        ("saturday", "SA"),
        ("sunday", "SU"),
    ] {
        if t.contains(name) {
            return Some(format!("FREQ=WEEKLY;BYDAY={code}"));
        }
    }
    if t.contains("weekly") || t.contains("every week") {
        let code = match weekday {
            Weekday::Mon => "MO",
            Weekday::Tue => "TU",
            Weekday::Wed => "WE",
            Weekday::Thu => "TH",
            Weekday::Fri => "FR",
            Weekday::Sat => "SA",
            Weekday::Sun => "SU",
        };
        return Some(format!("FREQ=WEEKLY;BYDAY={code}"));
    }
    None
}

fn parse_weekday(s: &str) -> Result<Weekday, RruleError> {
    match s {
        "MO" => Ok(Weekday::Mon),
        "TU" => Ok(Weekday::Tue),
        "WE" => Ok(Weekday::Wed),
        "TH" => Ok(Weekday::Thu),
        "FR" => Ok(Weekday::Fri),
        "SA" => Ok(Weekday::Sat),
        "SU" => Ok(Weekday::Sun),
        _ => Err(RruleError::InvalidSyntax(s.into())),
    }
}

/// Compute the next occurrence after `from` (exclusive), in the workspace
/// timezone `tz`. All weekday/date/BYHOUR arithmetic is done on the LOCAL
/// calendar (so "every Monday 9am" means 9am *local*, and the weekday is the
/// local weekday); the result is converted back to a UTC instant. Working on
/// naive local dates avoids DST pitfalls during the day-by-day walk, and the
/// final resolution handles a DST gap/fold at the target time.
pub fn next_occurrence(rule: &Rrule, from: DateTime<Utc>, tz: Tz) -> Option<DateTime<Utc>> {
    let from_local: NaiveDateTime = from.with_timezone(&tz).naive_local();
    let base_date: NaiveDate = from_local.date();

    // Time-of-day of each occurrence: BYHOUR/BYMINUTE if set, else `from`'s.
    let hour = rule.by_hour.unwrap_or_else(|| from_local.hour());
    let minute = rule.by_minute.unwrap_or_else(|| from_local.minute());
    let tod = NaiveTime::from_hms_opt(hour, minute, 0)?;

    let limit_days = match rule.freq {
        Freq::Daily => 31 * rule.interval as i64,
        Freq::Weekly => 7 * rule.interval as i64 * 4, // up to ~4 cycles
        Freq::Monthly => 366,
        Freq::Yearly => 366 * 4,
    };

    let mut date = base_date;
    for _ in 0..=limit_days {
        let candidate = NaiveDateTime::new(date, tod);
        if candidate > from_local && matches_rule(rule, date, base_date) {
            return Some(local_naive_to_utc(tz, candidate));
        }
        date += Duration::days(1);
    }
    None
}

/// Does `date` (a local calendar day) satisfy the rule, relative to the local
/// `base` day? Time-of-day is handled by the caller's `candidate > from` guard.
fn matches_rule(rule: &Rrule, date: NaiveDate, base: NaiveDate) -> bool {
    match rule.freq {
        Freq::Daily => {
            let days = (date - base).num_days();
            days > 0 && (days as u32) % rule.interval == 0
        }
        Freq::Weekly => {
            if rule.by_day.is_empty() {
                return false;
            }
            if !rule.by_day.contains(&date.weekday()) {
                return false;
            }
            let days = (date - base).num_days();
            if days <= 0 {
                return false;
            }
            // INTERVAL applies to weeks.
            let weeks = (days / 7) as u32;
            weeks % rule.interval == 0
        }
        Freq::Monthly => rule.by_month_day.is_some_and(|mday| date.day() == mday),
        Freq::Yearly => match (rule.by_month, rule.by_month_day) {
            (Some(m), Some(mday)) => date.month() == m && date.day() == mday,
            _ => false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use chrono_tz::Europe::Paris;
    use chrono_tz::UTC;

    fn dt(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, min, 0).unwrap()
    }

    #[test]
    fn parse_daily() {
        let r = parse("FREQ=DAILY").unwrap();
        assert_eq!(r.freq, Freq::Daily);
        assert_eq!(r.interval, 1);
    }

    #[test]
    fn parse_weekly_monday() {
        let r = parse("FREQ=WEEKLY;BYDAY=MO").unwrap();
        assert_eq!(r.freq, Freq::Weekly);
        assert_eq!(r.by_day, vec![Weekday::Mon]);
    }

    #[test]
    fn parse_weekly_friday_every_2_with_hour() {
        let r = parse("FREQ=WEEKLY;BYDAY=FR;INTERVAL=2;BYHOUR=18").unwrap();
        assert_eq!(r.interval, 2);
        assert_eq!(r.by_day, vec![Weekday::Fri]);
        assert_eq!(r.by_hour, Some(18));
    }

    #[test]
    fn parse_monthly_first() {
        let r = parse("FREQ=MONTHLY;BYMONTHDAY=1").unwrap();
        assert_eq!(r.freq, Freq::Monthly);
        assert_eq!(r.by_month_day, Some(1));
    }

    #[test]
    fn parse_yearly_birthday() {
        let r = parse("FREQ=YEARLY;BYMONTH=3;BYMONTHDAY=15").unwrap();
        assert_eq!(r.freq, Freq::Yearly);
        assert_eq!(r.by_month, Some(3));
        assert_eq!(r.by_month_day, Some(15));
    }

    #[test]
    fn next_daily_tomorrow() {
        let r = parse("FREQ=DAILY").unwrap();
        let n = next_occurrence(&r, dt(2026, 4, 19, 10, 0), UTC).unwrap();
        assert_eq!(
            n.date_naive(),
            NaiveDate::from_ymd_opt(2026, 4, 20).unwrap()
        );
    }

    #[test]
    fn next_weekly_friday_from_thursday() {
        let r = parse("FREQ=WEEKLY;BYDAY=FR").unwrap();
        // 2026-04-23 is a Thursday
        let n = next_occurrence(&r, dt(2026, 4, 23, 10, 0), UTC).unwrap();
        assert_eq!(
            n.date_naive(),
            NaiveDate::from_ymd_opt(2026, 4, 24).unwrap()
        );
    }

    #[test]
    fn next_weekly_friday_from_friday_returns_next_friday() {
        let r = parse("FREQ=WEEKLY;BYDAY=FR").unwrap();
        // 2026-04-24 is a Friday at 10:00 ; next is 2026-05-01 (next FR)
        let n = next_occurrence(&r, dt(2026, 4, 24, 10, 0), UTC).unwrap();
        assert_eq!(n.date_naive(), NaiveDate::from_ymd_opt(2026, 5, 1).unwrap());
    }

    #[test]
    fn next_monthly_first_from_mid_month() {
        let r = parse("FREQ=MONTHLY;BYMONTHDAY=1").unwrap();
        let n = next_occurrence(&r, dt(2026, 4, 19, 10, 0), UTC).unwrap();
        assert_eq!(n.date_naive(), NaiveDate::from_ymd_opt(2026, 5, 1).unwrap());
    }

    #[test]
    fn next_yearly_birthday() {
        let r = parse("FREQ=YEARLY;BYMONTH=3;BYMONTHDAY=15").unwrap();
        let n = next_occurrence(&r, dt(2026, 4, 19, 10, 0), UTC).unwrap();
        assert_eq!(
            n.date_naive(),
            NaiveDate::from_ymd_opt(2027, 3, 15).unwrap()
        );
    }

    #[test]
    fn weekly_with_byhour_snaps_time() {
        let r = parse("FREQ=WEEKLY;BYDAY=MO;BYHOUR=9").unwrap();
        let n = next_occurrence(&r, dt(2026, 4, 19, 18, 0), UTC).unwrap(); // dimanche 18h
        assert_eq!(
            n.date_naive(),
            NaiveDate::from_ymd_opt(2026, 4, 20).unwrap()
        );
        assert_eq!(n.hour(), 9);
    }

    // ── timezone-aware behaviour ───────────────────────────────────────────

    #[test]
    fn byhour_is_local_so_utc_hour_reflects_offset() {
        // "Every Monday 9am" in Paris. From Sunday 2026-04-19 06:00 UTC.
        // Next Monday 2026-04-20 09:00 *Paris* (CEST, +02:00) == 07:00 UTC.
        let r = parse("FREQ=WEEKLY;BYDAY=MO;BYHOUR=9").unwrap();
        let n = next_occurrence(&r, dt(2026, 4, 19, 6, 0), Paris).unwrap();
        assert_eq!(
            n.date_naive(),
            NaiveDate::from_ymd_opt(2026, 4, 20).unwrap()
        );
        assert_eq!(n.hour(), 7); // 9am local == 7am UTC in summer
    }

    #[test]
    fn text_to_rrule_cases() {
        use Weekday::*;
        assert_eq!(text_to_rrule("daily", Mon).as_deref(), Some("FREQ=DAILY"));
        assert_eq!(
            text_to_rrule("every day", Mon).as_deref(),
            Some("FREQ=DAILY")
        );
        assert_eq!(
            text_to_rrule("every monday", Fri).as_deref(),
            Some("FREQ=WEEKLY;BYDAY=MO")
        );
        assert_eq!(
            text_to_rrule("every friday", Mon).as_deref(),
            Some("FREQ=WEEKLY;BYDAY=FR")
        );
        // bare "weekly" derives the day from the trigger weekday
        assert_eq!(
            text_to_rrule("weekly", Wed).as_deref(),
            Some("FREQ=WEEKLY;BYDAY=WE")
        );
        // already an RRULE → passed through (upper-cased)
        assert_eq!(
            text_to_rrule("FREQ=WEEKLY;BYDAY=TU", Mon).as_deref(),
            Some("FREQ=WEEKLY;BYDAY=TU")
        );
        // one-off / unknown → None
        assert_eq!(text_to_rrule("", Mon), None);
        assert_eq!(text_to_rrule("once in a while", Mon), None);
    }

    #[test]
    fn weekday_uses_local_day_not_utc() {
        // 2026-04-19 23:00 UTC is already Monday 2026-04-20 01:00 in Paris.
        // "Every Monday" must treat the local base day (Mon) as excluded and
        // return the FOLLOWING Monday (2026-04-27) — not this one.
        let r = parse("FREQ=WEEKLY;BYDAY=MO;BYHOUR=9").unwrap();
        let n_paris = next_occurrence(&r, dt(2026, 4, 19, 23, 0), Paris).unwrap();
        assert_eq!(
            n_paris.date_naive(),
            NaiveDate::from_ymd_opt(2026, 4, 27).unwrap()
        );
        // In UTC the base day is still Sunday, so the next Monday is 2026-04-20.
        let n_utc = next_occurrence(&r, dt(2026, 4, 19, 23, 0), UTC).unwrap();
        assert_eq!(
            n_utc.date_naive(),
            NaiveDate::from_ymd_opt(2026, 4, 20).unwrap()
        );
    }
}
