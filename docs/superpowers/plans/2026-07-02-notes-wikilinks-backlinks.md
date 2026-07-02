# Notes Wikilinks + Backlinks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add title-based `[[wikilinks]]` (with editor autocomplete) and a backlinks panel to Grumps' existing notes feature.

**Architecture:** A pure parser in `grumps-core` is the single source of truth for `[[link]]` extraction and title normalization. The worker indexes link edges into a title-keyed `note_links` table on every note write (covering web, API, and chat capture), and serves a `/links` endpoint that returns backlinks plus resolved outgoing links. The Leptos SPA renders `[[…]]` as navigable links in the note read view, shows a "Linked from" panel, and offers a `[[`-triggered title picker in the editor.

**Tech Stack:** Rust (workspace crates), `workers-rs` on Cloudflare Workers, D1 (per-workspace) via a REST client, Leptos 0.8 + Trunk (WASM SPA), JSON i18n dictionaries (14 locales).

## Global Constraints

- **i18n hard rule:** no user-facing literal strings in `.rs`/`.html`. Every user-visible string goes through an i18n key in all 14 locales (`en es pt-BR fr de it ru tr ar hi zh-CN ja ko id`). Add keys with a `scripts/add-*-keys.py` script following the existing pattern.
- **No new dependencies.** The parser is hand-written std-only (no `regex`) to keep the SPA WASM bundle small.
- **Attribution:** where logic is adapted from `blamouche/browsidian`, add `// adapted from blamouche/browsidian, used with permission`.
- **Link resolution is title-based:** case-insensitive on a normalized title (trim, collapse inner whitespace, lowercase). Untitled notes are not linkable. Ambiguous titles resolve to the most-recently-updated note. Content stores raw `[[Title]]` — never rewritten.
- **Test target:** native tests run with `--target x86_64-unknown-linux-gnu`.
- **Commands:**
  - Test a crate: `cargo test -p <pkg> --target x86_64-unknown-linux-gnu`
  - Workspace tests: `cargo test --workspace --lib --tests --target x86_64-unknown-linux-gnu`
  - Clippy (wasm crates): `cargo clippy -p grumps-worker --target wasm32-unknown-unknown` / `-p grumps-spa`
  - Build SPA: `cd crates/spa && trunk build --release`

---

## File Structure

- **Create** `crates/core/src/wikilink.rs` — parser, normalization, edge builder (pure, tested).
- **Modify** `crates/core/src/lib.rs` — register `pub mod wikilink;`.
- **Create** `migrations/workspace/0010_note_links.sql` — `title_norm` column + `note_links` table.
- **Modify** `crates/worker/src/migrations.rs` — register migration version 10.
- **Modify** `crates/worker/src/db/workspace.rs` — set `title_norm` + rebuild edges on write; add `get_note_links`.
- **Modify** `crates/core/src/dto.rs` — `NoteLinksResponse`, `LinkRef`, `OutgoingLink`.
- **Modify** `crates/worker/src/routes/notes.rs` — `note_links` handler.
- **Modify** `crates/worker/src/lib.rs` — register `GET /api/w/:slug/notes/:id/links`.
- **Modify** `crates/spa/src/api/{mod.rs,live.rs,demo.rs,types.rs}` — `get_note_links` + types.
- **Create** `crates/spa/src/components/wikilink.rs` — render helper.
- **Modify** `crates/spa/src/components/mod.rs` — register component.
- **Modify** `crates/spa/src/pages/note_editor.rs` — rendered read view, backlinks panel, `[[` autocomplete.
- **Create** `scripts/add-wikilink-keys.py` — i18n keys for all 14 locales.

---

## Task 1: Wikilink parser + edge builder in `grumps-core`

**Files:**
- Create: `crates/core/src/wikilink.rs`
- Modify: `crates/core/src/lib.rs` (add module)
- Test: inline `#[cfg(test)]` in `crates/core/src/wikilink.rs` (mirrors `note.rs`)

**Interfaces:**
- Produces:
  - `struct Wikilink { pub target: String, pub alias: Option<String>, pub start: usize, pub end: usize }`
  - `fn normalize_title(s: &str) -> String`
  - `fn extract_wikilinks(content: &str) -> Vec<Wikilink>`
  - `struct LinkEdge { pub to_title_norm: String, pub display: String }`
  - `fn link_edges(content: &str) -> Vec<LinkEdge>` (deduped by `to_title_norm`, preserves first `display`)

- [ ] **Step 1: Write the failing tests**

Create `crates/core/src/wikilink.rs`:

```rust
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

// implementations added in Step 3

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
```

- [ ] **Step 2: Register the module and run tests to verify they fail**

Add to `crates/core/src/lib.rs`, keeping alphabetical order (after `pub mod todo;` or wherever it fits):

```rust
pub mod wikilink;
```

Run: `cargo test -p grumps-core --target x86_64-unknown-linux-gnu wikilink`
Expected: FAIL — `normalize_title`/`extract_wikilinks`/`link_edges` not found.

- [ ] **Step 3: Implement the parser**

Insert above the `#[cfg(test)]` block in `crates/core/src/wikilink.rs`:

```rust
/// Trim, collapse internal whitespace runs to a single space, and lowercase.
pub fn normalize_title(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// Extract `[[target]]` / `[[target|alias]]` links, skipping inline `code`
/// spans and fenced ``` code blocks. Empty targets are ignored.
pub fn extract_wikilinks(content: &str) -> Vec<Wikilink> {
    let bytes = content.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    let mut in_fence = false;   // inside a ``` fenced block
    let mut in_inline = false;  // inside a `...` inline span

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
        edges.push(LinkEdge { to_title_norm: norm, display });
    }
    edges
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p grumps-core --target x86_64-unknown-linux-gnu wikilink`
Expected: PASS (7 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/wikilink.rs crates/core/src/lib.rs
git commit -m "✨ Add wikilink parser + edge builder in grumps-core"
```

---

## Task 2: `note_links` migration (version 10)

**Files:**
- Create: `migrations/workspace/0010_note_links.sql`
- Modify: `crates/worker/src/migrations.rs` (append registry entry)

**Interfaces:**
- Produces: `notes.title_norm` column; `note_links(from_id, to_title_norm, display)` table; registry version 10.

- [ ] **Step 1: Write the migration SQL**

Create `migrations/workspace/0010_note_links.sql`:

```sql
-- Normalized title for pure-SQL join resolution (Rust normalize_title() is the
-- authority; this lower(trim()) is only a best-effort backfill for old rows).
ALTER TABLE notes ADD COLUMN title_norm TEXT;
UPDATE notes SET title_norm = lower(trim(title)) WHERE title IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_notes_title_norm ON notes(title_norm);

-- Directed link edges: from_id links to a note with the given normalized title.
CREATE TABLE IF NOT EXISTS note_links (
  from_id       TEXT NOT NULL,
  to_title_norm TEXT NOT NULL,
  display       TEXT,
  PRIMARY KEY (from_id, to_title_norm)
);
CREATE INDEX IF NOT EXISTS idx_note_links_to ON note_links(to_title_norm);
CREATE INDEX IF NOT EXISTS idx_note_links_from ON note_links(from_id);
```

- [ ] **Step 2: Register the migration**

In `crates/worker/src/migrations.rs`, append to the `vec![...]` in `workspace_migrations()` after the version-9 entry:

```rust
        Migration {
            version: 10,
            name: "note_links",
            data: false,
            sql: include_str!("../../../migrations/workspace/0010_note_links.sql"),
        },
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo clippy -p grumps-worker --target wasm32-unknown-unknown`
Expected: PASS (no errors; `include_str!` resolves the new file).

- [ ] **Step 4: Commit**

```bash
git add migrations/workspace/0010_note_links.sql crates/worker/src/migrations.rs
git commit -m "✨ Add note_links migration (v10) with title_norm column"
```

---

## Task 3: Index link edges on note writes

**Files:**
- Modify: `crates/worker/src/db/workspace.rs` (`insert_note`, `update_note`, `delete_note`; add `rebuild_note_links`)

**Interfaces:**
- Consumes: `grumps_core::wikilink::{link_edges, normalize_title}`.
- Produces: after any note write, `notes.title_norm` is set and `note_links` rows for that note reflect its content.

- [ ] **Step 1: Add the edge-rebuild helper**

In `crates/worker/src/db/workspace.rs`, add this method inside the `impl` block (near the note methods around line 346):

```rust
    /// Replace all outgoing link edges for a note with those parsed from its
    /// content. Called after every insert/update so web, API, and chat-created
    /// notes are all indexed uniformly.
    async fn rebuild_note_links(&self, note_id: &str, content: &str) -> Result<()> {
        self.q(
            "DELETE FROM note_links WHERE from_id = ?1",
            vec![note_id.into()],
        )
        .await?;
        for edge in grumps_core::wikilink::link_edges(content) {
            self.q(
                "INSERT OR IGNORE INTO note_links (from_id, to_title_norm, display) VALUES (?1, ?2, ?3)",
                vec![note_id.into(), edge.to_title_norm.into(), edge.display.into()],
            )
            .await?;
        }
        Ok(())
    }
```

- [ ] **Step 2: Set `title_norm` and rebuild edges in `insert_note`**

Replace the body of `insert_note` (currently a single `INSERT` + `Ok(id)`) with:

```rust
        let id = uuid::Uuid::new_v4().to_string();
        let title_norm = grumps_core::wikilink::normalize_title(title);
        self.q("INSERT INTO notes (id, title, title_norm, content, source, created_by, created_at, updated_at) VALUES (?1, NULLIF(?2,''), NULLIF(?3,''), ?4, ?5, ?6, datetime('now'), datetime('now'))",
            vec![id.clone().into(), title.into(), title_norm.into(), content.into(), source.into(), created_by.into()]).await?;
        self.rebuild_note_links(&id, content).await?;
        Ok(id)
```

- [ ] **Step 3: Set `title_norm` and rebuild edges in `update_note`**

Replace the body of `update_note` with:

```rust
        let title_norm = grumps_core::wikilink::normalize_title(title);
        self.q("UPDATE notes SET title = NULLIF(?1,''), title_norm = NULLIF(?2,''), content = ?3, updated_at = datetime('now') WHERE id = ?4",
            vec![title.into(), title_norm.into(), content.into(), note_id.into()]).await?;
        self.rebuild_note_links(note_id, content).await?;
        Ok(())
```

- [ ] **Step 4: Clear edges in `delete_note`**

In `delete_note`, before (or after) the existing `DELETE FROM notes`, add:

```rust
        self.q("DELETE FROM note_links WHERE from_id = ?1", vec![note_id.into()]).await?;
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo clippy -p grumps-worker --target wasm32-unknown-unknown`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/worker/src/db/workspace.rs
git commit -m "✨ Index note_links + title_norm on note insert/update/delete"
```

---

## Task 4: `/links` DTOs, DB query, endpoint

**Files:**
- Modify: `crates/core/src/dto.rs` (response types)
- Modify: `crates/worker/src/db/workspace.rs` (`get_note_links`)
- Modify: `crates/worker/src/routes/notes.rs` (`note_links` handler)
- Modify: `crates/worker/src/lib.rs` (route registration)
- Test: `crates/core/src/dto.rs` inline test (serialization)

**Interfaces:**
- Produces:
  - `NoteLinksResponse { backlinks: Vec<LinkRef>, outgoing: Vec<OutgoingLink> }`
  - `LinkRef { id: String, title: Option<String> }`
  - `OutgoingLink { display: String, id: Option<String> }`
  - `WorkspaceDb::get_note_links(&self, note_id: &str) -> Result<NoteLinksResponse>`
  - `GET /api/w/:slug/notes/:id/links`

- [ ] **Step 1: Write the failing DTO serialization test**

In `crates/core/src/dto.rs`, add to the existing `#[cfg(test)] mod tests` (or create one at the end of the file if absent):

```rust
    #[test]
    fn note_links_response_serializes() {
        let r = super::NoteLinksResponse {
            backlinks: vec![super::LinkRef { id: "n1".into(), title: Some("Wifi".into()) }],
            outgoing: vec![
                super::OutgoingLink { display: "Wifi".into(), id: Some("n1".into()) },
                super::OutgoingLink { display: "Ghost".into(), id: None },
            ],
        };
        let j = serde_json::to_value(&r).unwrap();
        assert_eq!(j["backlinks"][0]["id"], "n1");
        assert_eq!(j["outgoing"][1]["id"], serde_json::Value::Null);
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p grumps-core --target x86_64-unknown-linux-gnu note_links_response`
Expected: FAIL — `NoteLinksResponse` not found.

- [ ] **Step 3: Add the DTOs**

In `crates/core/src/dto.rs` (near the other note DTOs), add:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct LinkRef {
    pub id: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct OutgoingLink {
    pub display: String,
    pub id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct NoteLinksResponse {
    pub backlinks: Vec<LinkRef>,
    pub outgoing: Vec<OutgoingLink>,
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p grumps-core --target x86_64-unknown-linux-gnu note_links_response`
Expected: PASS.

- [ ] **Step 5: Add the DB query method**

In `crates/worker/src/db/workspace.rs`, add to the `impl` block. Uses `extract_rows` the same way other query methods in this file do (match the existing import/usage for row extraction):

```rust
    /// Backlinks (notes linking to this one, by normalized title) and this
    /// note's outgoing links with resolved ids (most-recent note wins on
    /// duplicate titles; None = unresolved).
    pub async fn get_note_links(
        &self,
        note_id: &str,
    ) -> Result<grumps_core::dto::NoteLinksResponse> {
        // Backlinks: who points at THIS note's normalized title.
        let backlink_resp = self.q(
            "SELECT n.id AS id, n.title AS title \
             FROM note_links l JOIN notes n ON n.id = l.from_id \
             WHERE l.to_title_norm = (SELECT title_norm FROM notes WHERE id = ?1) \
               AND (SELECT title_norm FROM notes WHERE id = ?1) IS NOT NULL \
             ORDER BY n.updated_at DESC",
            vec![note_id.into()],
        )
        .await?;

        #[derive(serde::Deserialize)]
        struct BRow { id: String, title: Option<String> }
        let backlinks = crate::d1_rest::extract_rows::<BRow>(&backlink_resp)?
            .into_iter()
            .map(|r| grumps_core::dto::LinkRef { id: r.id, title: r.title })
            .collect();

        // Outgoing: this note's edges, each resolved to the most-recent match.
        let out_resp = self.q(
            "SELECT l.display AS display, \
                    (SELECT id FROM notes WHERE title_norm = l.to_title_norm \
                     ORDER BY updated_at DESC LIMIT 1) AS id \
             FROM note_links l WHERE l.from_id = ?1 \
             ORDER BY rowid",
            vec![note_id.into()],
        )
        .await?;

        #[derive(serde::Deserialize)]
        struct ORow { display: String, id: Option<String> }
        let outgoing = crate::d1_rest::extract_rows::<ORow>(&out_resp)?
            .into_iter()
            .map(|r| grumps_core::dto::OutgoingLink { display: r.display, id: r.id })
            .collect();

        Ok(grumps_core::dto::NoteLinksResponse { backlinks, outgoing })
    }
```

> Note for the implementer: confirm the row-extraction helper name/signature by
> matching an existing query method in this same file (e.g. how `get_notes`
> decodes rows). If this file uses a different decoder than
> `crate::d1_rest::extract_rows`, use that one — do not introduce a new pattern.

- [ ] **Step 6: Add the route handler**

In `crates/worker/src/routes/notes.rs`, add (mirroring `get_note`):

```rust
// ── GET /api/w/:slug/notes/:id/links ──────────────────────────────────────────

pub async fn note_links(req: Request, ctx: RouteContext<()>, m: Member) -> Result<Response> {
    let note_id = ctx
        .param("id")
        .ok_or_else(|| Error::RustError("missing id".into()))?
        .to_string();

    let client = d1_rest::D1RestClient::from_env(&ctx.env)?;
    let ws_db = db::WorkspaceDb::new(&client, m.ws.d1_database_id);
    let links = ws_db.get_note_links(&note_id).await?;

    middleware::with_cors(&req, Response::from_json(&links)?)
}
```

- [ ] **Step 7: Register the route**

In `crates/worker/src/lib.rs`, immediately after the `get_note` registration block (around line 148), add:

```rust
        .get_async(
            "/api/w/:slug/notes/:id/links",
            extract::route(routes::notes::note_links),
        )
```

- [ ] **Step 8: Verify it compiles**

Run: `cargo clippy -p grumps-worker --target wasm32-unknown-unknown`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add crates/core/src/dto.rs crates/worker/src/db/workspace.rs crates/worker/src/routes/notes.rs crates/worker/src/lib.rs
git commit -m "✨ Add GET notes/:id/links (backlinks + resolved outgoing)"
```

---

## Task 5: SPA API client — `get_note_links`

**Files:**
- Modify: `crates/spa/src/api/types.rs` (client-side types)
- Modify: `crates/spa/src/api/mod.rs` (trait method)
- Modify: `crates/spa/src/api/live.rs` (HTTP impl)
- Modify: `crates/spa/src/api/demo.rs` (stub)

**Interfaces:**
- Produces: `Api::get_note_links(&self, slug: &str, id: &str) -> Result<NoteLinks, String>` where `NoteLinks { backlinks: Vec<LinkRef>, outgoing: Vec<OutgoingLink> }`.

- [ ] **Step 1: Add client types**

In `crates/spa/src/api/types.rs`, add (mirror the DTO; derive whatever the sibling types in this file derive — typically `Clone, Debug, serde::Deserialize`):

```rust
#[derive(Clone, Debug, serde::Deserialize)]
pub struct LinkRef {
    pub id: String,
    pub title: Option<String>,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct OutgoingLink {
    pub display: String,
    pub id: Option<String>,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct NoteLinks {
    pub backlinks: Vec<LinkRef>,
    pub outgoing: Vec<OutgoingLink>,
}
```

- [ ] **Step 2: Add the trait method**

In `crates/spa/src/api/mod.rs`, inside the `// Notes` block of the `Api` trait, after `delete_note`:

```rust
    async fn get_note_links(&self, slug: &str, id: &str) -> Result<NoteLinks, String>;
```

Ensure `NoteLinks` is in scope (add to the `use` of `types::…` at the top of `mod.rs` alongside `NoteItem`).

- [ ] **Step 3: Implement for the live client**

In `crates/spa/src/api/live.rs`, after `delete_note`:

```rust
    async fn get_note_links(&self, slug: &str, id: &str) -> Result<NoteLinks, String> {
        self.get(&format!("/api/w/{}/notes/{}/links", slug, id)).await
    }
```

- [ ] **Step 4: Implement the demo stub**

In `crates/spa/src/api/demo.rs`, after `delete_note`:

```rust
    async fn get_note_links(&self, _slug: &str, _id: &str) -> Result<NoteLinks, String> {
        Ok(NoteLinks { backlinks: vec![], outgoing: vec![] })
    }
```

(Add `NoteLinks` to the `use` imports in both `live.rs` and `demo.rs` matching how `NoteItem` is imported there.)

- [ ] **Step 5: Verify it compiles**

Run: `cargo clippy -p grumps-spa --target wasm32-unknown-unknown`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/spa/src/api/
git commit -m "✨ SPA API: get_note_links client method + types"
```

---

## Task 6: i18n keys for wikilink UI

**Files:**
- Create: `scripts/add-wikilink-keys.py`
- Modify: `crates/i18n/locales/*.json` (via the script)

**Interfaces:**
- Produces keys: `page.note_editor.backlinks_heading`, `page.note_editor.wikilink_unresolved`, `page.note_editor.wikilink_create`, `page.note_editor.link_picker_empty`.

- [ ] **Step 1: Write the key script**

Create `scripts/add-wikilink-keys.py` (mirrors `scripts/add-note-editor-keys.py`):

```python
#!/usr/bin/env python3
"""Localize wikilink/backlink UI strings."""
import json, pathlib

KEYS = {
    "page.note_editor.backlinks_heading": {
        "en": "Linked from", "es": "Enlazado desde", "pt-BR": "Vinculado de",
        "fr": "Lié depuis", "de": "Verlinkt von", "it": "Collegato da",
        "ru": "Ссылаются", "tr": "Bağlantı verenler", "ar": "مرتبط من",
        "hi": "यहाँ से लिंक", "zh-CN": "被链接自", "ja": "リンク元",
        "ko": "링크한 노트", "id": "Ditautkan dari",
    },
    "page.note_editor.wikilink_unresolved": {
        "en": "This note doesn't exist yet", "es": "Esta nota aún no existe",
        "pt-BR": "Esta nota ainda não existe", "fr": "Cette note n'existe pas encore",
        "de": "Diese Notiz existiert noch nicht", "it": "Questa nota non esiste ancora",
        "ru": "Такой заметки пока нет", "tr": "Bu not henüz yok",
        "ar": "هذه الملاحظة غير موجودة بعد", "hi": "यह नोट अभी मौजूद नहीं है",
        "zh-CN": "此笔记尚不存在", "ja": "このノートはまだありません",
        "ko": "아직 없는 노트예요", "id": "Catatan ini belum ada",
    },
    "page.note_editor.wikilink_create": {
        "en": "Create note “{title}”?", "es": "¿Crear nota «{title}»?",
        "pt-BR": "Criar nota “{title}”?", "fr": "Créer la note « {title} » ?",
        "de": "Notiz „{title}“ erstellen?", "it": "Creare la nota «{title}»?",
        "ru": "Создать заметку «{title}»?", "tr": "“{title}” notu oluşturulsun mu?",
        "ar": "إنشاء ملاحظة ‏«{title}»؟", "hi": "नोट “{title}” बनाएँ?",
        "zh-CN": "创建笔记“{title}”?", "ja": "ノート「{title}」を作成しますか？",
        "ko": "노트 “{title}”를 만들까요?", "id": "Buat catatan “{title}”?",
    },
    "page.note_editor.link_picker_empty": {
        "en": "No matching notes", "es": "Sin notas coincidentes",
        "pt-BR": "Nenhuma nota correspondente", "fr": "Aucune note correspondante",
        "de": "Keine passenden Notizen", "it": "Nessuna nota corrispondente",
        "ru": "Нет подходящих заметок", "tr": "Eşleşen not yok",
        "ar": "لا توجد ملاحظات مطابقة", "hi": "कोई मिलती नोट नहीं",
        "zh-CN": "没有匹配的笔记", "ja": "一致するノートがありません",
        "ko": "일치하는 노트가 없어요", "id": "Tidak ada catatan yang cocok",
    },
}

locales_dir = pathlib.Path(__file__).resolve().parent.parent / "crates/i18n/locales"
for lang_file in sorted(locales_dir.glob("*.json")):
    lang = lang_file.stem
    data = json.loads(lang_file.read_text(encoding="utf-8"))
    for key, translations in KEYS.items():
        data[key] = translations.get(lang, translations["en"])
    lang_file.write_text(
        json.dumps(data, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(f"updated {lang_file.name}")
```

> Note for the implementer: open `scripts/add-note-editor-keys.py` and match its
> exact writer style (indent, `sort_keys`, trailing newline) so the diff to the
> locale files stays minimal. Adjust the writer loop above if that script writes
> differently.

- [ ] **Step 2: Run the script**

Run: `python3 scripts/add-wikilink-keys.py`
Expected: prints `updated <lang>.json` for all 14 locales.

- [ ] **Step 3: Verify keys landed**

Run: `grep -c 'page.note_editor.backlinks_heading' crates/i18n/locales/*.json`
Expected: every file reports `1`.

- [ ] **Step 4: Commit**

```bash
git add scripts/add-wikilink-keys.py crates/i18n/locales/
git commit -m "🌐 Add i18n keys for wikilink/backlink UI (14 locales)"
```

---

## Task 7: Render wikilinks + backlinks panel in the note read view

**Files:**
- Create: `crates/spa/src/components/wikilink.rs`
- Modify: `crates/spa/src/components/mod.rs` (register)
- Modify: `crates/spa/src/pages/note_editor.rs` (fetch `/links`, replace read view, add panel)

**Interfaces:**
- Consumes: `grumps_core::wikilink::{extract_wikilinks, normalize_title}`, `Api::get_note_links`, i18n keys from Task 6.
- Produces: `render_note_content(content: String, resolver: std::collections::HashMap<String, String>, slug: String) -> AnyView` where `resolver` maps `normalize_title(target) -> note_id`.

- [ ] **Step 1: Write the render helper**

Create `crates/spa/src/components/wikilink.rs`:

```rust
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
                let href = format!("/w/{}/notes/{}", slug, id);
                nodes.push(view! {
                    <a href=href class="text-ink underline decoration-dotted font-semibold">{label}</a>
                }.into_any());
            }
            None => {
                nodes.push(view! {
                    <span class="text-ink/50 underline decoration-dotted"
                          title=tr("page.note_editor.wikilink_unresolved")>
                        {label}
                    </span>
                }.into_any());
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
```

- [ ] **Step 2: Register the component module**

In `crates/spa/src/components/mod.rs`, add alongside the other `pub mod` lines:

```rust
pub mod wikilink;
```

- [ ] **Step 3: Verify the helper compiles**

Run: `cargo clippy -p grumps-spa --target wasm32-unknown-unknown`
Expected: PASS.

- [ ] **Step 4: Fetch `/links` in the note editor page**

In `crates/spa/src/pages/note_editor.rs`, after the existing `note` `LocalResource` (around line 24), add a resource for links:

```rust
    let api_for_links = use_api();
    let links = LocalResource::new(move || {
        let api = api_for_links.clone();
        let s = slug();
        let id = note_id();
        async move { api.get_note_links(&s, &id).await.ok() }
    });
```

- [ ] **Step 5: Replace the raw `<pre>` read view with rendered content**

In `note_editor.rs`, find the read-view branch (the `else` arm currently rendering
`<pre class="whitespace-pre-wrap text-sm">{content.get()}</pre>`) and replace that
inner `<pre>` with a call into the helper, building the resolver from the links
resource:

```rust
                                view! {
                                    <div class="prose max-w-none p-6 border-2 border-ink rounded-xs" style="background: var(--cream-light);">
                                        {move || {
                                            let slug_v = slug();
                                            let resolver: std::collections::HashMap<String, String> =
                                                links.get().flatten().map(|l| {
                                                    l.outgoing.into_iter()
                                                        .filter_map(|o| o.id.map(|id| (grumps_core::wikilink::normalize_title(&o.display), id)))
                                                        .collect()
                                                }).unwrap_or_default();
                                            crate::components::wikilink::render_note_content(content.get(), resolver, slug_v)
                                        }}
                                    </div>
                                }.into_any()
```

> Resolver note: `outgoing[].display` is the alias-or-target text, and the
> resolver keys on `normalize_title(display)`. For plain `[[Title]]` links
> (no alias) this equals `normalize_title(target)`, so resolution is correct.
> Aliased links `[[Target|Alias]]` are rendered by the read view using their own
> `target` for resolution via the helper's `resolver` lookup — if you want
> aliased links resolvable too, key the resolver on the parsed `target` instead
> by extending `OutgoingLink` with a `target_norm` field. For v1, plain links
> (the common case) resolve; aliased links render but may show unresolved. This
> is an accepted v1 limitation; note it in the PR.

- [ ] **Step 6: Add the backlinks panel below the content**

Immediately after the `</div>` that closes the content block (still inside the
read-view container), add:

```rust
                            {move || {
                                let bl = links.get().flatten().map(|l| l.backlinks).unwrap_or_default();
                                if bl.is_empty() {
                                    ().into_any()
                                } else {
                                    let slug_v = slug();
                                    view! {
                                        <div class="mt-6">
                                            <h3 class="font-display text-sm mb-2">{tr("page.note_editor.backlinks_heading")}</h3>
                                            <ul class="flex flex-col gap-1">
                                                {bl.into_iter().map(|r| {
                                                    let href = format!("/w/{}/notes/{}", slug_v, r.id);
                                                    let label = r.title.unwrap_or_else(|| tr("page.note_editor.untitled"));
                                                    view! { <li><a href=href class="text-sm underline">{label}</a></li> }
                                                }).collect::<Vec<_>>()}
                                            </ul>
                                        </div>
                                    }.into_any()
                                }
                            }}
```

Ensure `use crate::i18n::tr;` is present in `note_editor.rs` (it already uses `tr`).

- [ ] **Step 7: Verify the SPA builds**

Run: `cd crates/spa && trunk build && cd ../..`
Expected: build succeeds with no errors.

- [ ] **Step 8: Commit**

```bash
git add crates/spa/src/components/wikilink.rs crates/spa/src/components/mod.rs crates/spa/src/pages/note_editor.rs
git commit -m "✨ Render wikilinks + backlinks panel in note read view"
```

---

## Task 8: `[[` autocomplete in the note editor

**Files:**
- Modify: `crates/spa/src/pages/note_editor.rs` (title picker on `[[`)

**Interfaces:**
- Consumes: `Api::get_notes` (existing), i18n `page.note_editor.link_picker_empty`.
- Produces: typing `[[` in the content textarea opens a title dropdown; selecting inserts `[[Title]]`.

- [ ] **Step 1: Load the workspace note titles for suggestions**

In `note_editor.rs`, add near the other resources:

```rust
    let api_for_titles = use_api();
    let all_notes = LocalResource::new(move || {
        let api = api_for_titles.clone();
        let s = slug();
        async move { api.get_notes(&s).await.unwrap_or_default() }
    });
    // Current [[partial query, or None when the picker is closed.
    let (link_query, set_link_query) = signal(None::<String>);
```

- [ ] **Step 2: Detect `[[` as the user types**

Replace the content `<textarea>`'s `on:input` handler with one that also updates
the picker state by inspecting the text just before the caret:

```rust
                                                on:input=move |ev| {
                                                    let v = event_target_value(&ev);
                                                    set_content.set(v.clone());
                                                    // Open the picker when the caret sits in an unclosed [[…
                                                    match v.rsplit_once("[[") {
                                                        Some((_, tail)) if !tail.contains("]]") && !tail.contains('\n') => {
                                                            set_link_query.set(Some(tail.to_string()));
                                                        }
                                                        _ => set_link_query.set(None),
                                                    }
                                                }
```

- [ ] **Step 3: Render the suggestion dropdown**

Directly after the content `<Field>…</Field>` block, add:

```rust
                                        {move || {
                                            match link_query.get() {
                                                None => ().into_any(),
                                                Some(q) => {
                                                    let qn = grumps_core::wikilink::normalize_title(&q);
                                                    let matches: Vec<_> = all_notes.get().unwrap_or_default()
                                                        .into_iter()
                                                        .filter_map(|n| n.title.clone().map(|t| (t, n)))
                                                        .filter(|(t, _)| grumps_core::wikilink::normalize_title(t).contains(&qn))
                                                        .take(8)
                                                        .collect();
                                                    if matches.is_empty() {
                                                        view! { <div class="text-sm text-ink/50 px-2 py-1">{tr("page.note_editor.link_picker_empty")}</div> }.into_any()
                                                    } else {
                                                        view! {
                                                            <ul class="border-2 border-ink rounded-xs mt-1 bg-cream-light max-h-48 overflow-y-auto">
                                                                {matches.into_iter().map(|(title, _)| {
                                                                    let title_for_click = title.clone();
                                                                    view! {
                                                                        <li>
                                                                            <button
                                                                                class="w-full text-left px-2 py-1 text-sm hover:bg-cream"
                                                                                on:click=move |_| {
                                                                                    set_content.update(|c| {
                                                                                        if let Some(idx) = c.rfind("[[") {
                                                                                            c.truncate(idx);
                                                                                            c.push_str(&format!("[[{}]]", title_for_click));
                                                                                        }
                                                                                    });
                                                                                    set_link_query.set(None);
                                                                                }
                                                                            >{title}</button>
                                                                        </li>
                                                                    }
                                                                }).collect::<Vec<_>>()}
                                                            </ul>
                                                        }.into_any()
                                                    }
                                                }
                                            }
                                        }}
```

> Behavior note: this inserts `[[Title]]` by replacing from the last `[[` to the
> caret-region end. It assumes the `[[` being completed is the last `[[` in the
> buffer, which holds while actively typing a link. This is the pragmatic v1
> (matches Browsidian's simple picker); a caret-index-precise version can come
> later. Document the limitation in the PR.

- [ ] **Step 4: Verify the SPA builds**

Run: `cd crates/spa && trunk build && cd ../..`
Expected: build succeeds.

- [ ] **Step 5: Commit**

```bash
git add crates/spa/src/pages/note_editor.rs
git commit -m "✨ Add [[ ]] title autocomplete to the note editor"
```

---

## Task 9: Final verification

**Files:** none (verification only).

- [ ] **Step 1: Format**

Run: `cargo fmt --all`
Then: `git diff --stat` — if fmt changed files, review and include them.

- [ ] **Step 2: Full native test suite**

Run: `cargo test --workspace --lib --tests --target x86_64-unknown-linux-gnu`
Expected: PASS, including the new `wikilink` and `note_links_response` tests.

- [ ] **Step 3: Clippy (native + both wasm crates)**

Run:
```bash
cargo clippy --workspace --all-targets --target x86_64-unknown-linux-gnu
cargo clippy -p grumps-worker --target wasm32-unknown-unknown
cargo clippy -p grumps-spa    --target wasm32-unknown-unknown
```
Expected: no warnings/errors.

- [ ] **Step 4: SPA release build**

Run: `cd crates/spa && trunk build --release && cd ../..`
Expected: success.

- [ ] **Step 5: Runtime smoke (manual, documented for the reviewer)**

Because the DB layer talks to D1 over REST, do a live smoke after deploy of the
worker with migrations applied (`POST /api/admin/migrate-all` per
`crates/worker/src/migrations.rs` docs):
1. Create note "wifi" and a note containing `see [[wifi]]`.
2. Open the second note — `[[wifi]]` renders as a link and navigates to the first.
3. Open "wifi" — the backlinks panel lists the second note.
4. In the editor, type `[[wi` — the picker suggests "wifi".

- [ ] **Step 6: Commit any fmt changes**

```bash
git add -A
git commit -m "🎨 cargo fmt + final verification for wikilinks/backlinks" || echo "nothing to commit"
```

---

## Self-Review Notes (already applied)

- **Spec coverage:** §3 resolution rules → Tasks 1, 3, 4. §4 shared parser → Task 1. §5 index/migration/queries → Tasks 2–4. §6 SPA render/autocomplete/panel → Tasks 5, 7, 8. §7 i18n → Task 6. §8 testing → Tasks 1, 4, 9.
- **Known v1 limitations (documented in-plan, accepted in spec §9):** aliased-link resolution keys on display text; the `[[` picker completes the last open `[[`. Both are called out in Tasks 7–8 for the PR description.
- **Cross-task type consistency:** `NoteLinksResponse/LinkRef/OutgoingLink` (core dto, Task 4) mirrored as `NoteLinks/LinkRef/OutgoingLink` (spa types, Task 5); `render_note_content` signature is fixed in Task 7 and consumed there only.
