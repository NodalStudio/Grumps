//! Pure classification for the SPA's demo-mode gate.
//!
//! Split out from `crates/spa/src/demo.rs` (which is wasm-bound via
//! `web_sys::window()`) so the string-matching logic is natively testable.
//! `crates/spa/src/demo.rs::is_demo()` just reads `window.location` and
//! delegates the decision here.
//!
//! Demo mode short-circuits every API call to seed data and bypasses the
//! auth gate — so it must never trigger on a real workspace slug. The
//! earlier implementation used substring matching (`pathname.contains("/demo")`,
//! `search.contains("demo=1")`), which could false-positive on a workspace
//! slugged e.g. `demolition-crew` or on an unrelated query string that
//! happens to contain the text `demo=1`. Exact segment/param parsing closes
//! that gap.

/// `true` when `pathname` has a path segment exactly equal to `"demo"` —
/// covers both a root-mounted SPA (`/demo/...`) and one served under a
/// prefix (`/Grumps/demo/...` on GH Pages) — or when `search`'s `demo`
/// query parameter is exactly `"1"`.
pub fn classify_demo(pathname: &str, search: &str) -> bool {
    if pathname.split('/').any(|seg| seg == "demo") {
        return true;
    }
    query_param(search, "demo").as_deref() == Some("1")
}

/// Find the `"demo"` path segment and return the prefix up to and
/// including it, e.g. `"/Grumps/demo/w/x"` -> `"/Grumps/demo"`. Returns
/// `None` if `pathname` has no `"demo"` segment.
pub fn demo_router_base(pathname: &str) -> Option<String> {
    let segments: Vec<&str> = pathname.split('/').collect();
    let idx = segments.iter().position(|s| *s == "demo")?;
    Some(segments[..=idx].join("/"))
}

/// Parse `key` out of a query string (with or without the leading `?`).
/// Exact key match, first occurrence wins — no substring matching.
fn query_param(search: &str, key: &str) -> Option<String> {
    let q = search.strip_prefix('?').unwrap_or(search);
    for pair in q.split('&') {
        if pair.is_empty() {
            continue;
        }
        let mut it = pair.splitn(2, '=');
        if it.next() == Some(key) {
            return Some(it.next().unwrap_or("").to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_mounted_demo_path_matches() {
        assert!(classify_demo("/demo", ""));
        assert!(classify_demo("/demo/", ""));
        assert!(classify_demo("/demo/w/roommates/todos", ""));
    }

    #[test]
    fn gh_pages_prefixed_demo_path_matches() {
        assert!(classify_demo("/Grumps/demo/", ""));
        assert!(classify_demo("/Grumps/demo/w/roommates", ""));
    }

    #[test]
    fn workspace_slug_starting_with_demo_does_not_match() {
        // The bug this closes: a workspace slugged "demolition-crew" must
        // never be treated as demo mode.
        assert!(!classify_demo("/w/demolition-crew", ""));
        assert!(!classify_demo("/w/demolition-crew/todos", ""));
    }

    #[test]
    fn unrelated_paths_do_not_match() {
        assert!(!classify_demo("/w/roommates", ""));
        assert!(!classify_demo("/dashboard", ""));
        assert!(!classify_demo("/", ""));
        assert!(!classify_demo("", ""));
    }

    #[test]
    fn query_param_exact_one_matches() {
        assert!(classify_demo("/w/roommates", "?demo=1"));
        assert!(classify_demo("/w/roommates", "demo=1"));
        assert!(classify_demo("/w/roommates", "?lang=fr&demo=1"));
        assert!(classify_demo("/w/roommates", "?demo=1&lang=fr"));
    }

    #[test]
    fn query_param_non_exact_does_not_match() {
        assert!(!classify_demo("/w/roommates", "?demo=true"));
        assert!(!classify_demo("/w/roommates", "?demo=10"));
        assert!(!classify_demo("/w/roommates", "?demolition=1"));
        assert!(!classify_demo("/w/roommates", "?demo="));
        assert!(!classify_demo("/w/roommates", ""));
    }

    #[test]
    fn router_base_extracts_prefix_up_to_demo_segment() {
        assert_eq!(demo_router_base("/demo/w/x").as_deref(), Some("/demo"));
        assert_eq!(
            demo_router_base("/Grumps/demo/w/x").as_deref(),
            Some("/Grumps/demo")
        );
        assert_eq!(demo_router_base("/w/roommates"), None);
    }
}
