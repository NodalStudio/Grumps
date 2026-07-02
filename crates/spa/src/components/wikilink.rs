//! Render note markdown with `[[wikilinks]]` turned into navigable links.
//! Only linkifies wikilinks — full markdown rendering is out of scope (§9.4).
//! adapted from blamouche/browsidian, used with permission.

use crate::i18n::tr;
use grumps_core::wikilink::{extract_wikilinks, normalize_title};
use leptos::prelude::*;
use std::collections::HashMap;

/// `resolver`: normalized target title -> existing note id.
pub fn render_note_content(
    content: String,
    resolver: HashMap<String, String>,
    slug: String,
) -> AnyView {
    let links = extract_wikilinks(&content);
    let mut nodes: Vec<AnyView> = Vec::new();
    let mut cursor = 0usize;

    for link in links {
        if link.start > cursor {
            nodes.push(view! { <span>{content[cursor..link.start].to_string()}</span> }.into_any());
        }
        let label = link.alias.clone().unwrap_or_else(|| link.target.clone());
        match resolver.get(&normalize_title(&link.target)) {
            Some(id) => {
                let href = format!("{}/w/{}/notes/{}", crate::demo::router_base(), slug, id);
                nodes.push(
                    view! {
                        <a href=href class="text-ink underline decoration-dotted font-semibold">{label}</a>
                    }
                    .into_any(),
                );
            }
            None => {
                nodes.push(
                    view! {
                        <span class="text-ink/50 underline decoration-dotted"
                              title=tr("page.note_editor.wikilink_unresolved")>
                            {label}
                        </span>
                    }
                    .into_any(),
                );
            }
        }
        cursor = link.end;
    }
    if cursor < content.len() {
        nodes.push(view! { <span>{content[cursor..].to_string()}</span> }.into_any());
    }

    view! {
        <pre class="whitespace-pre-wrap text-sm">{nodes}</pre>
    }
    .into_any()
}
