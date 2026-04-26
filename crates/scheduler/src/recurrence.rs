//! Minimal RRULE expander supporting the 5 launch-time cases :
//! - FREQ=DAILY
//! - FREQ=WEEKLY;BYDAY=...
//! - FREQ=WEEKLY;BYDAY=...;INTERVAL=N
//! - FREQ=MONTHLY;BYMONTHDAY=N
//! - FREQ=YEARLY;BYMONTH=M;BYMONTHDAY=N
//!
//! See spec § 7.6.

use chrono::{DateTime, Utc, Datelike, Weekday, Duration, Timelike};
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
    pub interval: u32,                      // default 1
    pub by_day: Vec<Weekday>,               // for WEEKLY
    pub by_month_day: Option<u32>,          // for MONTHLY/YEARLY
    pub by_month: Option<u32>,              // for YEARLY
    pub by_hour: Option<u32>,               // optional for any FREQ
    pub by_minute: Option<u32>,             // optional for any FREQ
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
        let k = kv.next().ok_or_else(|| RruleError::InvalidSyntax(rrule.into()))?;
        let v = kv.next().ok_or_else(|| RruleError::InvalidSyntax(rrule.into()))?;
        match k {
            "FREQ" => freq = Some(match v {
                "DAILY" => Freq::Daily,
                "WEEKLY" => Freq::Weekly,
                "MONTHLY" => Freq::Monthly,
                "YEARLY" => Freq::Yearly,
                other => return Err(RruleError::UnsupportedFreq(other.into())),
            }),
            "INTERVAL" => interval = v.parse().map_err(|_| RruleError::InvalidSyntax(v.into()))?,
            "BYDAY" => by_day = v.split(',').map(parse_weekday).collect::<Result<Vec<_>, _>>()?,
            "BYMONTHDAY" => by_month_day = Some(v.parse().map_err(|_| RruleError::InvalidSyntax(v.into()))?),
            "BYMONTH" => by_month = Some(v.parse().map_err(|_| RruleError::InvalidSyntax(v.into()))?),
            "BYHOUR" => by_hour = Some(v.parse().map_err(|_| RruleError::InvalidSyntax(v.into()))?),
            "BYMINUTE" => by_minute = Some(v.parse().map_err(|_| RruleError::InvalidSyntax(v.into()))?),
            _ => { /* ignore unknown */ }
        }
    }

    Ok(Rrule {
        freq: freq.ok_or_else(|| RruleError::MissingField("FREQ".into()))?,
        interval, by_day, by_month_day, by_month, by_hour, by_minute,
    })
}

fn parse_weekday(s: &str) -> Result<Weekday, RruleError> {
    match s {
        "MO" => Ok(Weekday::Mon), "TU" => Ok(Weekday::Tue),
        "WE" => Ok(Weekday::Wed), "TH" => Ok(Weekday::Thu),
        "FR" => Ok(Weekday::Fri), "SA" => Ok(Weekday::Sat),
        "SU" => Ok(Weekday::Sun),
        _ => Err(RruleError::InvalidSyntax(s.into())),
    }
}

/// Compute the next occurrence after `from` (exclusive).
pub fn next_occurrence(rule: &Rrule, from: DateTime<Utc>) -> Option<DateTime<Utc>> {
    // Strategy : starting at `from + 1 minute`, walk day by day for up to
    // a finite limit, find the first datetime matching the rule.
    let limit_days = match rule.freq {
        Freq::Daily => 31 * rule.interval as i64,
        Freq::Weekly => 7 * rule.interval as i64 * 4,    // up to ~4 cycles
        Freq::Monthly => 366,
        Freq::Yearly => 366 * 4,
    };
    let mut candidate = from + Duration::minutes(1);
    // Snap time to BYHOUR/BYMINUTE if specified
    if let Some(h) = rule.by_hour {
        candidate = candidate.with_hour(h)?.with_minute(rule.by_minute.unwrap_or(0))?
            .with_second(0)?.with_nanosecond(0)?;
        if candidate <= from { candidate = candidate + Duration::days(1); }
    }
    for _ in 0..limit_days {
        if matches_rule(rule, candidate, from) {
            return Some(candidate);
        }
        candidate = candidate + Duration::days(1);
        if let Some(h) = rule.by_hour {
            candidate = candidate.with_hour(h)?.with_minute(rule.by_minute.unwrap_or(0))?
                .with_second(0)?.with_nanosecond(0)?;
        }
    }
    None
}

fn matches_rule(rule: &Rrule, dt: DateTime<Utc>, base: DateTime<Utc>) -> bool {
    match rule.freq {
        Freq::Daily => {
            let days_since_base = (dt.date_naive() - base.date_naive()).num_days();
            days_since_base > 0 && (days_since_base as u32) % rule.interval == 0
        }
        Freq::Weekly => {
            if rule.by_day.is_empty() { return false; }
            if !rule.by_day.contains(&dt.weekday()) { return false; }
            let days_since_base = (dt.date_naive() - base.date_naive()).num_days();
            if days_since_base <= 0 { return false; }
            // INTERVAL applies to weeks
            let weeks_since_base = (days_since_base / 7) as u32;
            weeks_since_base % rule.interval == 0
        }
        Freq::Monthly => {
            if let Some(mday) = rule.by_month_day {
                dt.day() == mday
            } else { false }
        }
        Freq::Yearly => {
            if let (Some(m), Some(mday)) = (rule.by_month, rule.by_month_day) {
                dt.month() == m && dt.day() == mday
            } else { false }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let n = next_occurrence(&r, dt(2026, 4, 19, 10, 0)).unwrap();
        assert_eq!(n.date_naive(), NaiveDate::from_ymd_opt(2026, 4, 20).unwrap());
    }

    #[test]
    fn next_weekly_friday_from_thursday() {
        let r = parse("FREQ=WEEKLY;BYDAY=FR").unwrap();
        // 2026-04-23 is a Thursday
        let n = next_occurrence(&r, dt(2026, 4, 23, 10, 0)).unwrap();
        assert_eq!(n.date_naive(), NaiveDate::from_ymd_opt(2026, 4, 24).unwrap());
    }

    #[test]
    fn next_weekly_friday_from_friday_returns_next_friday() {
        let r = parse("FREQ=WEEKLY;BYDAY=FR").unwrap();
        // 2026-04-24 is a Friday at 10:00 ; next is 2026-05-01 (next FR)
        let n = next_occurrence(&r, dt(2026, 4, 24, 10, 0)).unwrap();
        assert_eq!(n.date_naive(), NaiveDate::from_ymd_opt(2026, 5, 1).unwrap());
    }

    #[test]
    fn next_monthly_first_from_mid_month() {
        let r = parse("FREQ=MONTHLY;BYMONTHDAY=1").unwrap();
        let n = next_occurrence(&r, dt(2026, 4, 19, 10, 0)).unwrap();
        assert_eq!(n.date_naive(), NaiveDate::from_ymd_opt(2026, 5, 1).unwrap());
    }

    #[test]
    fn next_yearly_birthday() {
        let r = parse("FREQ=YEARLY;BYMONTH=3;BYMONTHDAY=15").unwrap();
        let n = next_occurrence(&r, dt(2026, 4, 19, 10, 0)).unwrap();
        assert_eq!(n.date_naive(), NaiveDate::from_ymd_opt(2027, 3, 15).unwrap());
    }

    #[test]
    fn weekly_with_byhour_snaps_time() {
        let r = parse("FREQ=WEEKLY;BYDAY=MO;BYHOUR=9").unwrap();
        let n = next_occurrence(&r, dt(2026, 4, 19, 18, 0)).unwrap();  // dimanche 18h
        assert_eq!(n.date_naive(), NaiveDate::from_ymd_opt(2026, 4, 20).unwrap());
        assert_eq!(n.hour(), 9);
    }
}
