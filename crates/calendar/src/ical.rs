//! RFC 5545 VCALENDAR generator.
//! See spec § 9.4.

use crate::aggregate::CalendarItem;

/// Fold a content line per RFC 5545 §3.1: break at 75 octets, continue with `\r\n ` (space).
fn fold_line(line: &str) -> String {
    if line.len() <= 75 {
        return line.to_string();
    }
    let mut out = String::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut first = true;
    while i < bytes.len() {
        // Take up to 75 octets for first chunk, 74 for continuation (leading space takes 1)
        let max = if first { 75 } else { 74 };
        let mut end = (i + max).min(bytes.len());
        // Back off to avoid splitting a UTF-8 multibyte sequence
        while end < bytes.len() && (bytes[end] & 0xC0) == 0x80 {
            end -= 1;
        }
        if !first {
            out.push_str("\r\n ");
        }
        out.push_str(std::str::from_utf8(&bytes[i..end]).unwrap_or(""));
        i = end;
        first = false;
    }
    out
}

/// Push a folded property line followed by CRLF.
fn push_prop(out: &mut String, line: String) {
    out.push_str(&fold_line(&line));
    out.push_str("\r\n");
}

pub fn generate_ical(workspace_name: &str, items: &[CalendarItem], workspace_tz: Option<&str>) -> String {
    let mut out = String::new();
    out.push_str("BEGIN:VCALENDAR\r\n");
    out.push_str("VERSION:2.0\r\n");
    out.push_str("PRODID:-//Grumps//Calendar 1.0//EN\r\n");
    push_prop(&mut out, format!("NAME:Grumps \u{2014} {workspace_name}"));
    push_prop(&mut out, format!("X-WR-CALNAME:Grumps \u{2014} {workspace_name}"));
    // Default calendar timezone for the subscriber's client (helps all-day /
    // floating items render against the workspace's calendar).
    if let Some(tz) = workspace_tz.filter(|t| !t.is_empty()) {
        push_prop(&mut out, format!("X-WR-TIMEZONE:{tz}"));
    }
    out.push_str("X-PUBLISHED-TTL:PT15M\r\n");

    for item in items {
        out.push_str("BEGIN:VEVENT\r\n");
        push_prop(&mut out, format!("UID:{}@grumps.app", item.id));
        push_prop(
            &mut out,
            format!("DTSTAMP:{}", format_ical_dt(&chrono::Utc::now())),
        );
        if item.all_day {
            push_prop(
                &mut out,
                format!("DTSTART;VALUE=DATE:{}", format_ical_date(&item.starts_at)),
            );
        } else {
            push_prop(
                &mut out,
                format!("DTSTART:{}", format_ical_dt(&item.starts_at)),
            );
            if let Some(end) = &item.ends_at {
                push_prop(&mut out, format!("DTEND:{}", format_ical_dt(end)));
            }
        }
        push_prop(&mut out, format!("SUMMARY:{}", escape_ical(&item.title)));
        if let Some(loc) = &item.location {
            push_prop(&mut out, format!("LOCATION:{}", escape_ical(loc)));
        }
        if let Some(rrule) = &item.recurrence {
            push_prop(&mut out, format!("RRULE:{}", rrule));
        }
        push_prop(&mut out, format!("URL:{}", item.url));
        out.push_str("END:VEVENT\r\n");
    }
    out.push_str("END:VCALENDAR\r\n");
    out
}

fn format_ical_dt(dt: &chrono::DateTime<chrono::Utc>) -> String {
    dt.format("%Y%m%dT%H%M%SZ").to_string()
}

fn format_ical_date(dt: &chrono::DateTime<chrono::Utc>) -> String {
    dt.format("%Y%m%d").to_string()
}

fn escape_ical(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregate::{CalendarItem, CalendarSource};
    use chrono::TimeZone;

    #[test]
    fn empty_generates_valid_envelope() {
        let s = generate_ical("Test", &[], Some("Europe/Paris"));
        assert!(s.contains("X-WR-TIMEZONE:Europe/Paris"));
        assert!(s.starts_with("BEGIN:VCALENDAR"));
        assert!(s.ends_with("END:VCALENDAR\r\n"));
        assert!(s.contains("PRODID:-//Grumps//Calendar 1.0//EN"));
    }

    #[test]
    fn vevent_includes_uid_dtstart_summary() {
        let item = CalendarItem {
            id: "evt:1".into(),
            source: CalendarSource::Event,
            title: "R\u{e9}union".into(),
            starts_at: chrono::Utc.with_ymd_and_hms(2026, 4, 20, 10, 0, 0).unwrap(),
            ends_at: None,
            all_day: false,
            location: Some("R\u{e9}publique".into()),
            color: "teal".into(),
            member_id: None,
            recurrence: Some("FREQ=WEEKLY;BYDAY=MO".into()),
            editable: true,
            url: "/w/ws1/events/1".into(),
        };
        let s = generate_ical("Test", &[item], None);
        assert!(s.contains("UID:evt:1@grumps.app"));
        assert!(s.contains("DTSTART:20260420T100000Z"));
        assert!(s.contains("SUMMARY:R\u{e9}union"));
        assert!(s.contains("LOCATION:R\u{e9}publique"));
        assert!(s.contains("RRULE:FREQ=WEEKLY;BYDAY=MO"));
    }

    #[test]
    fn long_summary_is_folded() {
        let item = CalendarItem {
            id: "evt:1".into(), source: CalendarSource::Event,
            title: "A very long event title that exceeds the seventy-five octet limit imposed by RFC 5545 section 3.1".into(),
            starts_at: chrono::Utc.with_ymd_and_hms(2026, 4, 20, 10, 0, 0).unwrap(),
            ends_at: None, all_day: false, location: None, color: "teal".into(),
            member_id: None, recurrence: None, editable: true, url: "/x".into(),
        };
        let s = generate_ical("Test", &[item], None);
        // Each physical line (split on \n) should be <= 75 octets
        for line in s.split('\n') {
            let line = line.trim_end_matches('\r');
            assert!(
                line.len() <= 75,
                "line too long: {} bytes — {line}",
                line.len()
            );
        }
        // Content should survive folding
        assert!(s.contains("seventy-five octet limit"));
    }

    #[test]
    fn escapes_special_chars() {
        let item = CalendarItem {
            id: "evt:2".into(),
            source: CalendarSource::Event,
            title: "Hello; world, hi".into(),
            starts_at: chrono::Utc.with_ymd_and_hms(2026, 4, 20, 10, 0, 0).unwrap(),
            ends_at: None,
            all_day: false,
            location: None,
            color: "teal".into(),
            member_id: None,
            recurrence: None,
            editable: true,
            url: "/x".into(),
        };
        let s = generate_ical("X", &[item], None);
        assert!(s.contains(r"SUMMARY:Hello\; world\, hi"));
    }
}
