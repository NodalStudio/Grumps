//! Tiny query-string helpers — avoid pulling serde_urlencoded.

use worker::Url;

/// Read query params from a URL into a struct of choice.
/// Caller passes a closure that pattern-matches on (key, value) pairs.
pub fn read_query<F>(url: &Url, mut f: F)
where
    F: FnMut(&str, &str),
{
    for (k, v) in url.query_pairs() {
        f(k.as_ref(), v.as_ref());
    }
}
