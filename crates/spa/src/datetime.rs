//! Timestamp formatting in the **workspace** timezone — never the browser's.
//!
//! All instants come from the API as UTC RFC3339 ("2026-05-31T18:00:00Z"). We
//! render them in the workspace's IANA timezone using the browser's built-in
//! `Intl`/`Date` (which already ships a full tz database) so we don't bloat the
//! WASM bundle with chrono-tz. The browser's own timezone is used ONLY for the
//! one-time auto-detection of a workspace's default — never for display.

use leptos::prelude::*;
use wasm_bindgen::JsValue;

/// Reactive workspace timezone (IANA name, e.g. "Europe/Paris"), provided at
/// the workspace layout so every renderer formats against the same zone.
#[derive(Clone, Copy)]
pub struct TimezoneSignal(pub RwSignal<String>);

/// Current workspace timezone, or "UTC" if not yet loaded / out of context.
pub fn use_timezone() -> String {
    use_context::<TimezoneSignal>()
        .map(|s| s.0.get())
        .unwrap_or_else(|| "UTC".to_string())
}

/// Install the timezone signal into context. Returns it so the loader can
/// update it once the workspace info arrives.
pub fn provide_timezone(initial: impl Into<String>) -> RwSignal<String> {
    let sig = RwSignal::new(initial.into());
    provide_context(TimezoneSignal(sig));
    sig
}

/// The browser's own IANA timezone — for one-time detection only.
pub fn browser_timezone() -> Option<String> {
    let dtf = js_sys::Intl::DateTimeFormat::new(&js_sys::Array::new(), &js_sys::Object::new());
    let resolved = dtf.resolved_options();
    js_sys::Reflect::get(&resolved, &JsValue::from_str("timeZone"))
        .ok()?
        .as_string()
        .filter(|s| !s.is_empty())
}

fn set_opt(obj: &js_sys::Object, key: &str, val: &str) {
    let _ = js_sys::Reflect::set(obj, &JsValue::from_str(key), &JsValue::from_str(val));
}

fn render(date: &js_sys::Date, locale: &str, opts: &js_sys::Object, fallback: &str) -> String {
    if date.get_time().is_nan() {
        return fallback.to_string();
    }
    let opts_val: JsValue = opts.clone().into();
    date.to_locale_string(locale, &opts_val)
        .as_string()
        .unwrap_or_else(|| fallback.to_string())
}

/// Format a UTC instant as date + time in `tz` (e.g. "May 31, 2026, 20:00").
pub fn format_instant(utc_iso: &str, tz: &str, locale: &str) -> String {
    let date = js_sys::Date::new(&JsValue::from_str(utc_iso));
    let opts = js_sys::Object::new();
    set_opt(&opts, "timeZone", tz);
    set_opt(&opts, "year", "numeric");
    set_opt(&opts, "month", "short");
    set_opt(&opts, "day", "numeric");
    set_opt(&opts, "hour", "2-digit");
    set_opt(&opts, "minute", "2-digit");
    render(&date, locale, &opts, utc_iso)
}

/// Format just the wall-clock time of a UTC instant in `tz` (e.g. "20:00").
pub fn format_time(utc_iso: &str, tz: &str, locale: &str) -> String {
    let date = js_sys::Date::new(&JsValue::from_str(utc_iso));
    let opts = js_sys::Object::new();
    set_opt(&opts, "timeZone", tz);
    set_opt(&opts, "hour", "2-digit");
    set_opt(&opts, "minute", "2-digit");
    render(&date, locale, &opts, utc_iso)
}

/// The civil date key "YYYY-MM-DD" of a UTC instant *as seen in `tz`* — for
/// grouping/sorting timeline items by the workspace's calendar day. Use this
/// for timed instants; all-day items already carry a bare date (don't shift).
pub fn date_key_in_tz(utc_iso: &str, tz: &str) -> String {
    let fallback = utc_iso.get(0..10).unwrap_or(utc_iso).to_string();
    let date = js_sys::Date::new(&JsValue::from_str(utc_iso));
    let opts = js_sys::Object::new();
    set_opt(&opts, "timeZone", tz);
    set_opt(&opts, "year", "numeric");
    set_opt(&opts, "month", "2-digit");
    set_opt(&opts, "day", "2-digit");
    // en-CA renders as "YYYY-MM-DD".
    render(&date, "en-CA", &opts, &fallback)
}

/// The workspace-calendar-day key "YYYY-MM-DD" for a calendar item, applying the
/// single project convention: **all-day items are floating civil dates** (their
/// bare date, never tz-shifted) while **timed instants are bucketed in the
/// workspace tz**. Every calendar view groups/filters through this so month,
/// week, agenda and overview always agree.
pub fn item_day_key(starts_at: &str, all_day: bool, tz: &str) -> String {
    if all_day {
        starts_at.get(..10).unwrap_or(starts_at).to_string()
    } else {
        date_key_in_tz(starts_at, tz)
    }
}

/// Today's civil date `(year, month, day)` in the workspace `tz` — never the
/// browser's. Use for "is this cell today?" highlighting and default views.
pub fn today_in_tz(tz: &str) -> (i32, u32, u32) {
    let now_iso = js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .unwrap_or_default();
    let key = date_key_in_tz(&now_iso, tz); // "YYYY-MM-DD"
    let mut it = key.split('-');
    let y = it.next().and_then(|s| s.parse().ok()).unwrap_or(1970);
    let m = it.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    let d = it.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    (y, m, d)
}

/// The wall-clock time of a UTC instant in `tz`, "HH:MM" (or "HH:MM:SS"), 24h.
/// Composed with [`date_key_in_tz`] rather than relying on a locale's
/// date+time separator.
fn time_in_tz(date: &js_sys::Date, tz: &str, with_seconds: bool) -> String {
    let opts = js_sys::Object::new();
    set_opt(&opts, "timeZone", tz);
    set_opt(&opts, "hour", "2-digit");
    set_opt(&opts, "minute", "2-digit");
    if with_seconds {
        set_opt(&opts, "second", "2-digit");
    }
    set_opt(&opts, "hourCycle", "h23");
    let opts_val: JsValue = opts.into();
    date.to_locale_string("en-GB", &opts_val)
        .as_string()
        .unwrap_or_else(|| {
            if with_seconds {
                "00:00:00".into()
            } else {
                "00:00".into()
            }
        })
}

/// Render the wall clock of a UTC instant in `tz` as a `datetime-local` input
/// value, "YYYY-MM-DDTHH:MM". Inverse of [`input_local_to_utc`].
pub fn to_input_local(utc_iso: &str, tz: &str) -> String {
    let date = js_sys::Date::new(&JsValue::from_str(utc_iso));
    if date.get_time().is_nan() {
        return utc_iso.get(..16).unwrap_or(utc_iso).to_string();
    }
    format!(
        "{}T{}",
        date_key_in_tz(utc_iso, tz),
        time_in_tz(&date, tz, false)
    )
}

/// The wall clock of `date` rendered in `tz`, then reinterpreted as UTC ms — used
/// to derive the tz's UTC offset at a given instant.
fn wall_as_utc_ms(tz: &str, date: &js_sys::Date) -> f64 {
    let utc_iso = date.to_iso_string().as_string().unwrap_or_default();
    let iso = format!(
        "{}T{}Z",
        date_key_in_tz(&utc_iso, tz),
        time_in_tz(date, tz, true)
    );
    js_sys::Date::new(&JsValue::from_str(&iso)).get_time()
}

/// Interpret a `datetime-local` value "YYYY-MM-DDTHH:MM" as a wall clock in `tz`
/// and convert it to a UTC instant "YYYY-MM-DDTHH:MM:SSZ". Inverse of
/// [`to_input_local`]. (The tz offset is sampled at the entered wall time, so a
/// time inside a DST transition can be off by the transition amount — the usual
/// ~1h ambiguity twice a year.)
pub fn input_local_to_utc(local: &str, tz: &str) -> String {
    if local.is_empty() {
        return String::new();
    }
    let provisional = js_sys::Date::new(&JsValue::from_str(&format!("{local}:00Z")));
    if provisional.get_time().is_nan() {
        return local.to_string();
    }
    let offset = wall_as_utc_ms(tz, &provisional) - provisional.get_time();
    let real = js_sys::Date::new_0();
    real.set_time(provisional.get_time() - offset);
    let iso = real.to_iso_string().as_string().unwrap_or_default();
    if iso.len() >= 19 {
        format!("{}Z", &iso[..19])
    } else {
        local.to_string()
    }
}

/// Format a **civil date** (all-day item / deadline) — NO timezone shift. A day
/// is a day; we format the stored calendar date as-is (anchored in UTC so the
/// displayed day always equals the stored day).
pub fn format_civil_date(iso: &str, locale: &str) -> String {
    let date_part = iso.get(0..10).unwrap_or(iso);
    let date = js_sys::Date::new(&JsValue::from_str(&format!("{date_part}T00:00:00Z")));
    let opts = js_sys::Object::new();
    set_opt(&opts, "timeZone", "UTC");
    set_opt(&opts, "weekday", "short");
    set_opt(&opts, "month", "short");
    set_opt(&opts, "day", "numeric");
    render(&date, locale, &opts, iso)
}
