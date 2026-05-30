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
