//! SPA-side i18n glue. Wraps `grumps_i18n::Locale` in a Leptos signal
//! so components rerender when the user switches language, and exposes
//! ergonomic `tr()` / `tr_p()` / `tr_n()` helpers.
//!
//! Locale priority (first hit wins):
//!   1. `?lang=` URL query
//!   2. `localStorage.locale`
//!   3. `navigator.language`
//!   4. `Locale::En`

use grumps_i18n::Locale;
use leptos::prelude::*;

/// Reactive current locale.
#[derive(Clone, Copy)]
pub struct LocaleSignal(pub RwSignal<Locale>);

pub fn use_locale() -> Locale {
    let sig = use_context::<LocaleSignal>()
        .expect("LocaleSignal not provided — wrap your component tree with provide_locale()");
    sig.0.get()
}

/// Translate `key` in the current locale.
pub fn tr(key: &str) -> String {
    grumps_i18n::t(use_locale(), key, &[])
}

/// Translate `key` with `{var}` substitutions.
pub fn tr_p(key: &str, params: &[(&str, &str)]) -> String {
    grumps_i18n::t(use_locale(), key, params)
}

/// Plural-aware translate — picks `key.{plural-category}` based on the
/// current locale's CLDR rule. See `grumps_i18n::plural_category`.
pub fn tr_n(key_base: &str, n: i64, params: &[(&str, &str)]) -> String {
    grumps_i18n::t_plural(use_locale(), key_base, n, params)
}

/// Translate an API error code (the `Err` string returned by `api::Api`
/// methods — see `api::live::LiveApi`), falling back to a generic "save
/// failed" toast when the code isn't a real i18n key (e.g. a network
/// failure or an unparseable error body, which surface as `http_{status}`).
pub fn tr_err(code: &str) -> String {
    if grumps_i18n::has_key(code) {
        tr(code)
    } else {
        tr("toast.save_failed")
    }
}

/// Initial detection — runs once at boot.
fn detect_initial_locale() -> Locale {
    use gloo_storage::{LocalStorage, Storage};
    use web_sys::window;

    if let Some(win) = window() {
        if let Ok(href) = win.location().href() {
            if let Some(q) = href.split('?').nth(1) {
                // Strip the hash fragment so `lang=ja#/foo` parses as `ja`.
                let q = q.split('#').next().unwrap_or(q);
                for pair in q.split('&') {
                    let mut kv = pair.splitn(2, '=');
                    if kv.next() == Some("lang") {
                        if let Some(v) = kv.next() {
                            return Locale::from_code(v);
                        }
                    }
                }
            }
        }
    }
    if let Ok(v) = LocalStorage::get::<String>("locale") {
        return Locale::from_code(&v);
    }
    if let Some(win) = window() {
        if let Some(nav_lang) = win.navigator().language() {
            return Locale::from_code(&nav_lang);
        }
    }
    Locale::En
}

/// Install the locale signal into Leptos context. Call once from `App`.
/// Also wires `<html lang="…" dir="…">` and persists to localStorage.
pub fn provide_locale() {
    let initial = detect_initial_locale();
    let sig = RwSignal::new(initial);
    provide_context(LocaleSignal(sig));

    Effect::new(move |_| {
        let l = sig.get();
        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
            if let Some(html) = doc.document_element() {
                let _ = html.set_attribute("lang", l.code());
                let _ = html.set_attribute("dir", if l.is_rtl() { "rtl" } else { "ltr" });
            }
        }
        use gloo_storage::{LocalStorage, Storage};
        let _ = LocalStorage::set("locale", l.code().to_string());
    });
}

/// Imperatively set the current locale. Call from the lang switcher.
pub fn set_locale(l: Locale) {
    if let Some(sig) = use_context::<LocaleSignal>() {
        sig.0.set(l);
    }
}
