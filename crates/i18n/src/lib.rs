//! Shared i18n for Grumps — used by the worker (bot canned messages, OG
//! tags, error toasts) and the SPA (every visible string).
//!
//! Why not Fluent? Fluent ships ~150 KB of code (regex, ICU plurals)
//! which is heavy for the SPA WASM bundle. Our messages are short and
//! don't need ICU plurals — a flat `{key} -> string` JSON map with
//! simple `{var}` substitution is enough.

use once_cell::sync::Lazy;
use serde_json::Value;
use std::collections::HashMap;

/// All locales Grumps supports. Order matches priority for fallback chains.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Locale {
    En, Es, PtBr, Fr, De, It, Ru, Tr, Ar, Hi, ZhCn, Ja, Ko, Id,
}

impl Locale {
    pub const ALL: &'static [Locale] = &[
        Locale::En, Locale::Es, Locale::PtBr, Locale::Fr, Locale::De,
        Locale::It, Locale::Ru, Locale::Tr, Locale::Ar, Locale::Hi,
        Locale::ZhCn, Locale::Ja, Locale::Ko, Locale::Id,
    ];

    /// BCP-47 code (`en`, `pt-BR`, `zh-CN`, …).
    pub fn code(self) -> &'static str {
        match self {
            Locale::En => "en", Locale::Es => "es", Locale::PtBr => "pt-BR",
            Locale::Fr => "fr", Locale::De => "de", Locale::It => "it",
            Locale::Ru => "ru", Locale::Tr => "tr", Locale::Ar => "ar",
            Locale::Hi => "hi", Locale::ZhCn => "zh-CN", Locale::Ja => "ja",
            Locale::Ko => "ko", Locale::Id => "id",
        }
    }

    pub fn english_name(self) -> &'static str {
        match self {
            Locale::En => "English", Locale::Es => "Spanish",
            Locale::PtBr => "Portuguese (Brazil)", Locale::Fr => "French",
            Locale::De => "German", Locale::It => "Italian",
            Locale::Ru => "Russian", Locale::Tr => "Turkish",
            Locale::Ar => "Arabic", Locale::Hi => "Hindi",
            Locale::ZhCn => "Chinese (Simplified)", Locale::Ja => "Japanese",
            Locale::Ko => "Korean", Locale::Id => "Indonesian",
        }
    }

    pub fn native_name(self) -> &'static str {
        match self {
            Locale::En => "English", Locale::Es => "Español",
            Locale::PtBr => "Português (BR)", Locale::Fr => "Français",
            Locale::De => "Deutsch", Locale::It => "Italiano",
            Locale::Ru => "Русский", Locale::Tr => "Türkçe",
            Locale::Ar => "العربية", Locale::Hi => "हिन्दी",
            Locale::ZhCn => "简体中文", Locale::Ja => "日本語",
            Locale::Ko => "한국어", Locale::Id => "Bahasa Indonesia",
        }
    }

    pub fn is_rtl(self) -> bool { matches!(self, Locale::Ar) }

    pub fn from_code(s: &str) -> Locale {
        for l in Self::ALL { if l.code() == s { return *l; } }
        let primary = s.split(['-', '_']).next().unwrap_or("");
        match primary {
            "en" => Locale::En, "es" => Locale::Es, "pt" => Locale::PtBr,
            "fr" => Locale::Fr, "de" => Locale::De, "it" => Locale::It,
            "ru" => Locale::Ru, "tr" => Locale::Tr, "ar" => Locale::Ar,
            "hi" => Locale::Hi, "zh" => Locale::ZhCn, "ja" => Locale::Ja,
            "ko" => Locale::Ko, "id" => Locale::Id,
            _ => Locale::En,
        }
    }

    /// Best locale match from an `Accept-Language`-style header value.
    pub fn from_accept_language(header: &str) -> Locale {
        for tag in header.split(',') {
            let tag = tag.split(';').next().unwrap_or("").trim();
            if tag.is_empty() { continue; }
            let l = Locale::from_code(tag);
            if l != Locale::En || tag.starts_with("en") { return l; }
        }
        Locale::En
    }
}

const LOCALE_FILES: &[(Locale, &str)] = &[
    (Locale::En,   include_str!("../locales/en.json")),
    (Locale::Es,   include_str!("../locales/es.json")),
    (Locale::PtBr, include_str!("../locales/pt-BR.json")),
    (Locale::Fr,   include_str!("../locales/fr.json")),
    (Locale::De,   include_str!("../locales/de.json")),
    (Locale::It,   include_str!("../locales/it.json")),
    (Locale::Ru,   include_str!("../locales/ru.json")),
    (Locale::Tr,   include_str!("../locales/tr.json")),
    (Locale::Ar,   include_str!("../locales/ar.json")),
    (Locale::Hi,   include_str!("../locales/hi.json")),
    (Locale::ZhCn, include_str!("../locales/zh-CN.json")),
    (Locale::Ja,   include_str!("../locales/ja.json")),
    (Locale::Ko,   include_str!("../locales/ko.json")),
    (Locale::Id,   include_str!("../locales/id.json")),
];

static BUNDLES: Lazy<HashMap<Locale, HashMap<String, String>>> = Lazy::new(|| {
    let mut out = HashMap::new();
    for (locale, raw) in LOCALE_FILES {
        let parsed: Value = serde_json::from_str(raw)
            .unwrap_or_else(|e| panic!("invalid JSON in locale {}: {e}", locale.code()));
        let map: HashMap<String, String> = parsed
            .as_object()
            .map(|obj| obj.iter().filter_map(|(k, v)| {
                v.as_str().map(|s| (k.clone(), s.to_string()))
            }).collect())
            .unwrap_or_default();
        out.insert(*locale, map);
    }
    out
});

/// Look up `key` in `locale`, applying `{var}` substitutions from `params`.
/// Falls back to English if missing in `locale`. Returns the key itself
/// if missing in English too — surfaces typos quickly.
pub fn t(locale: Locale, key: &str, params: &[(&str, &str)]) -> String {
    let raw = BUNDLES.get(&locale)
        .and_then(|m| m.get(key))
        .or_else(|| BUNDLES.get(&Locale::En).and_then(|m| m.get(key)))
        .map(String::as_str)
        .unwrap_or(key);
    if params.is_empty() { return raw.to_string(); }
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            let mut name = String::new();
            while let Some(&nc) = chars.peek() {
                if nc == '}' { chars.next(); break; }
                name.push(nc);
                chars.next();
            }
            let trimmed = name.trim();
            if let Some((_, v)) = params.iter().find(|(k, _)| *k == trimmed) {
                out.push_str(v);
            } else {
                out.push('{'); out.push_str(&name); out.push('}');
            }
        } else { out.push(c); }
    }
    out
}

/// CLDR cardinal plural categories. Returns the suffix that should be
/// appended to a base translation key (`schedule.fired` → `schedule.fired.one`,
/// `.few`, `.many`, `.other`, etc.) so the bundle picks the correct form
/// for the locale's grammar.
///
/// We implement enough rules to cover the 14 supported locales correctly:
///   - English-family (en/de/es/pt-BR/it/tr/hi/id):  one (n=1)        | other
///   - French (fr):                                  one (n=0|1)      | other
///   - Russian (ru):                                 one | few | many
///   - Arabic (ar):                                  zero | one | two | few | many | other
///   - CJK (zh-CN/ja/ko):                            other (no distinction)
///
/// Source: <https://cldr.unicode.org/index/cldr-spec/plural-rules>
pub fn plural_category(locale: Locale, n: i64) -> &'static str {
    let n = n.unsigned_abs();
    match locale {
        // n = 0 or 1 → one. Common for fr.
        Locale::Fr => if n == 0 || n == 1 { "one" } else { "other" },

        // n = 1 → one. Most European + Hindi + Indonesian + Turkish.
        Locale::En | Locale::De | Locale::Es | Locale::PtBr |
        Locale::It | Locale::Tr | Locale::Hi | Locale::Id => {
            if n == 1 { "one" } else { "other" }
        }

        // East Asian: no plural distinction.
        Locale::ZhCn | Locale::Ja | Locale::Ko => "other",

        // Russian (Slavic family): one/few/many.
        Locale::Ru => {
            let mod10 = n % 10;
            let mod100 = n % 100;
            if mod10 == 1 && mod100 != 11 { "one" }
            else if (2..=4).contains(&mod10) && !(12..=14).contains(&mod100) { "few" }
            else { "many" }
        }

        // Arabic: zero/one/two/few/many/other.
        Locale::Ar => {
            if n == 0 { "zero" }
            else if n == 1 { "one" }
            else if n == 2 { "two" }
            else {
                let mod100 = n % 100;
                if (3..=10).contains(&mod100) { "few" }
                else if (11..=99).contains(&mod100) { "many" }
                else { "other" }
            }
        }
    }
}

/// Translate `key_base.{plural-category}` for `n`, with `{n}` auto-injected
/// into params. Falls through to English if the per-locale variant is
/// missing, then to the literal key. Use this whenever a string varies
/// by count.
///
/// Example keys for `key_base = "schedule.fired"`:
///
/// ```json
/// "schedule.fired.one":   "Fired {n} time",
/// "schedule.fired.other": "Fired {n} times",
/// "schedule.fired.few":   "Запущено {n} раза",
/// "schedule.fired.many":  "Запущено {n} раз"
/// ```
pub fn t_plural(locale: Locale, key_base: &str, n: i64, params: &[(&str, &str)]) -> String {
    let cat = plural_category(locale, n);
    let key = format!("{key_base}.{cat}");
    let n_str = n.to_string();
    let mut all_params: Vec<(&str, &str)> = params.to_vec();
    all_params.push(("n", &n_str));
    let direct = t(locale, &key, &all_params);
    if direct == key {
        // Per-category key missing — try `.other` as a safe fallback before
        // surfacing the literal key (which means a translator skipped this entry).
        let other_key = format!("{key_base}.other");
        if other_key != key {
            return t(locale, &other_key, &all_params);
        }
    }
    direct
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn locale_round_trip() { for l in Locale::ALL { assert_eq!(*l, Locale::from_code(l.code())); } }
    #[test] fn accept_language() {
        assert_eq!(Locale::Fr, Locale::from_accept_language("fr-FR,fr;q=0.9,en;q=0.8"));
        assert_eq!(Locale::Ja, Locale::from_accept_language("ja,en;q=0.5"));
        assert_eq!(Locale::PtBr, Locale::from_accept_language("pt-PT,pt;q=0.9"));
        assert_eq!(Locale::En, Locale::from_accept_language(""));
    }
    #[test] fn unknown_key() { assert_eq!("__missing", t(Locale::En, "__missing", &[])); }

    #[test] fn plural_english() {
        assert_eq!("one", plural_category(Locale::En, 1));
        assert_eq!("other", plural_category(Locale::En, 0));
        assert_eq!("other", plural_category(Locale::En, 2));
        assert_eq!("other", plural_category(Locale::En, 100));
    }
    #[test] fn plural_french() {
        // fr: 0 and 1 are "one", others "other"
        assert_eq!("one", plural_category(Locale::Fr, 0));
        assert_eq!("one", plural_category(Locale::Fr, 1));
        assert_eq!("other", plural_category(Locale::Fr, 2));
    }
    #[test] fn plural_russian() {
        assert_eq!("one", plural_category(Locale::Ru, 1));
        assert_eq!("one", plural_category(Locale::Ru, 21));
        assert_eq!("few", plural_category(Locale::Ru, 2));
        assert_eq!("few", plural_category(Locale::Ru, 23));
        assert_eq!("many", plural_category(Locale::Ru, 5));
        assert_eq!("many", plural_category(Locale::Ru, 11));   // teen → many
        assert_eq!("many", plural_category(Locale::Ru, 14));
    }
    #[test] fn plural_arabic() {
        assert_eq!("zero", plural_category(Locale::Ar, 0));
        assert_eq!("one", plural_category(Locale::Ar, 1));
        assert_eq!("two", plural_category(Locale::Ar, 2));
        assert_eq!("few", plural_category(Locale::Ar, 5));
        assert_eq!("many", plural_category(Locale::Ar, 50));
        assert_eq!("other", plural_category(Locale::Ar, 100));
    }
    #[test] fn plural_cjk_no_distinction() {
        assert_eq!("other", plural_category(Locale::Ja, 1));
        assert_eq!("other", plural_category(Locale::Ja, 100));
        assert_eq!("other", plural_category(Locale::ZhCn, 1));
    }
}
