//! RFC 5545 VCALENDAR generator.
//! See spec § 9.4.

use crate::aggregate::CalendarItem;

pub fn generate_ical(workspace_name: &str, items: &[CalendarItem]) -> String {
    let mut out = String::new();
    out.push_str("BEGIN:VCALENDAR\r\n");
    out.push_str("VERSION:2.0\r\n");
    out.push_str("PRODID:-//Grumps//Calendar 1.0//EN\r\n");
    out.push_str(&format!("NAME:Grumps \u{2014} {workspace_name}\r\n"));
    out.push_str(&format!("X-WR-CALNAME:Grumps \u{2014} {workspace_name}\r\n"));
    out.push_str("X-PUBLISHED-TTL:PT15M\r\n");

    for item in items {
        out.push_str("BEGIN:VEVENT\r\n");
        out.push_str(&format!("UID:{}@grumps.io\r\n", item.id));
        out.push_str(&format!("DTSTAMP:{}\r\n", format_ical_dt(&chrono::Utc::now())));
        if item.all_day {
            out.push_str(&format!("DTSTART;VALUE=DATE:{}\r\n", format_ical_date(&item.starts_at)));
        } else {
            out.push_str(&format!("DTSTART:{}\r\n", format_ical_dt(&item.starts_at)));
            if let Some(end) = &item.ends_at {
                out.push_str(&format!("DTEND:{}\r\n", format_ical_dt(end)));
            }
        }
        out.push_str(&format!("SUMMARY:{}\r\n", escape_ical(&item.title)));
        if let Some(loc) = &item.location {
            out.push_str(&format!("LOCATION:{}\r\n", escape_ical(loc)));
        }
        if let Some(rrule) = &item.recurrence {
            out.push_str(&format!("RRULE:{}\r\n", rrule));
        }
        out.push_str(&format!("URL:{}\r\n", item.url));
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
        let s = generate_ical("Test", &[]);
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
        let s = generate_ical("Test", &[item]);
        assert!(s.contains("UID:evt:1@grumps.io"));
        assert!(s.contains("DTSTART:20260420T100000Z"));
        assert!(s.contains("SUMMARY:R\u{e9}union"));
        assert!(s.contains("LOCATION:R\u{e9}publique"));
        assert!(s.contains("RRULE:FREQ=WEEKLY;BYDAY=MO"));
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
        let s = generate_ical("X", &[item]);
        assert!(s.contains(r"SUMMARY:Hello\; world\, hi"));
    }
}
