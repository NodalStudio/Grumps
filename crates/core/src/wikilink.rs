//! Parse Obsidian-style `[[wikilinks]]` and normalize note titles for matching.
//! Pure, std-only (no regex) so it stays cheap in the SPA WASM bundle and is
//! shared by both the worker (link indexing) and the SPA (rendering).
//! adapted from blamouche/browsidian, used with permission.

use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wikilink {
    pub target: String,
    pub alias: Option<String>,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkEdge {
    pub to_title_norm: String,
    pub display: String,
}

/// Trim, collapse internal whitespace runs to a single space, and lowercase.
pub fn normalize_title(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Extract `[[target]]` / `[[target|alias]]` links, skipping inline `code`
/// spans and fenced ``` code blocks. Empty targets are ignored.
pub fn extract_wikilinks(content: &str) -> Vec<Wikilink> {
    let bytes = content.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    let mut in_fence = false; // inside a ``` fenced block
    let mut in_inline = false; // inside a `...` inline span

    // Track line starts to detect fence markers.
    let mut at_line_start = true;

    while i < bytes.len() {
        // Fenced code: a line starting with ``` toggles the fence.
        if at_line_start && content[i..].starts_with("```") {
            in_fence = !in_fence;
            // advance to end of line
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            at_line_start = true;
            continue;
        }

        let c = bytes[i];
        at_line_start = c == b'\n';

        if in_fence {
            i += 1;
            continue;
        }

        if c == b'`' {
            in_inline = !in_inline;
            i += 1;
            continue;
        }

        if !in_inline && content[i..].starts_with("[[") {
            if let Some(rel_close) = content[i + 2..].find("]]") {
                let inner = &content[i + 2..i + 2 + rel_close];
                // A wikilink never spans a newline.
                if !inner.contains('\n') {
                    let (target_raw, alias) = match inner.split_once('|') {
                        Some((t, a)) => (t.trim(), Some(a.trim().to_string())),
                        None => (inner.trim(), None),
                    };
                    if !target_raw.is_empty() {
                        let end = i + 2 + rel_close + 2;
                        out.push(Wikilink {
                            target: target_raw.to_string(),
                            alias: alias.filter(|a| !a.is_empty()),
                            start: i,
                            end,
                        });
                        i = end;
                        at_line_start = false;
                        continue;
                    }
                }
            }
        }

        i += 1;
    }
    out
}

/// Deduplicated link edges for indexing: one per normalized target, keeping the
/// first occurrence's display text (alias if present, else the raw target).
pub fn link_edges(content: &str) -> Vec<LinkEdge> {
    let mut seen = std::collections::HashSet::new();
    let mut edges = Vec::new();
    for link in extract_wikilinks(content) {
        let norm = normalize_title(&link.target);
        if norm.is_empty() || !seen.insert(norm.clone()) {
            continue;
        }
        let display = link.alias.clone().unwrap_or_else(|| link.target.clone());
        edges.push(LinkEdge {
            to_title_norm: norm,
            display,
        });
    }
    edges
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_trims_collapses_lowercases() {
        assert_eq!(normalize_title("  Wi  Fi "), "wi fi");
        assert_eq!(normalize_title("WIFI"), "wifi");
        assert_eq!(normalize_title("café Notes"), "café notes");
    }

    #[test]
    fn extracts_plain_and_alias() {
        let links = extract_wikilinks("see [[wifi]] and [[Note|the note]] end");
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].target, "wifi");
        assert_eq!(links[0].alias, None);
        assert_eq!(links[1].target, "Note");
        assert_eq!(links[1].alias, Some("the note".to_string()));
    }

    #[test]
    fn spans_point_at_the_link() {
        let src = "x [[a]] y";
        let links = extract_wikilinks(src);
        assert_eq!(&src[links[0].start..links[0].end], "[[a]]");
    }

    #[test]
    fn ignores_empty_target() {
        assert!(extract_wikilinks("[[]] and [[ | alias ]]").is_empty());
    }

    #[test]
    fn skips_inline_code() {
        assert!(extract_wikilinks("use `[[notliteral]]` here").is_empty());
    }

    #[test]
    fn skips_fenced_code() {
        let src = "before\n```\n[[nope]]\n```\nafter [[yes]]";
        let links = extract_wikilinks(src);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "yes");
    }

    #[test]
    fn edges_dedupe_by_normalized_target() {
        let edges = link_edges("[[Wifi]] [[wifi]] [[Other|shown]]");
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].to_title_norm, "wifi");
        assert_eq!(edges[0].display, "Wifi");
        assert_eq!(edges[1].to_title_norm, "other");
        assert_eq!(edges[1].display, "shown");
    }
}
