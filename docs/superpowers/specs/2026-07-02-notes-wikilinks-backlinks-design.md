# Notes: Wikilinks + Backlinks — Design

**Date:** 2026-07-02
**Branch:** `feat/notes-wikilinks-backlinks`
**Status:** Draft for review

## 1. Goal

Bring two Obsidian-style capabilities to Grumps' existing notes feature:

1. **`[[Wikilinks]]` with autocomplete** — link one note to another by title, with
   a type-ahead picker in the editor.
2. **Backlinks panel** — on a note, show every note that links *to* it.

Scope is deliberately narrow. Folder/tree navigation and click-to-edit live
preview are explicitly **out of scope** (see §9). This is a native
Rust/Leptos implementation; UI/parse logic is adapted with permission from
[blamouche/browsidian](https://github.com/blamouche/browsidian) (credit in
code comments), not copied wholesale — the two stacks differ (Node/vanilla JS
vs Rust/WASM).

## 2. Current state (as-built)

- **Model** (`crates/core/src/note.rs`): `Note { id, title: Option<String>,
  content: String (markdown), pinned, source, created_by, created_at }`.
  Titles are **optional and not unique**.
- **DB** (`crates/worker/src/db/workspace.rs`, per-workspace D1): `notes` table
  with `title TEXT` (`NULLIF('')`), `content TEXT`, `tags TEXT`. Methods
  `insert_note / update_note / get_notes / get_note_by_id / delete_note`, all
  through `self.q(sql, params)`.
- **Routes** (`crates/worker/src/routes/notes.rs`): REST CRUD under
  `/api/w/:slug/notes`.
- **SPA** (`crates/spa`, Leptos/WASM): `pages/notes.rs` (list), `note_editor.rs`
  (title + content textarea, debounced autosave), `components/note_card.rs`.
  **No markdown rendering exists yet** — note content currently displays as raw
  text. §9.4 of SPECS plans a `pulldown-cmark` pipeline; it is not built.
- **Chat**: `NOTE:` capture creates notes via the same `insert_note` DB path.
- No `note_links` table exists.

## 3. Key decision — how a `[[link]]` resolves

**Chosen: title-based, Obsidian-style.** `[[wifi]]` targets the note titled
`wifi`. Rationale: chat-friendly (`@grumps note see [[wifi]]`), content stays
portable, matches the Obsidian mental model. Rules:

- **Matching** is case-insensitive on a **normalized title** (trim, collapse
  inner whitespace, lowercase). `[[Wifi]]`, `[[wifi]]`, `[[ WIFI ]]` all match a
  note titled "Wifi".
- **Alias syntax** `[[Title|shown text]]` — link resolves on `Title`, displays
  `shown text`.
- **Untitled notes are not linkable** (no title to match). Acceptable — untitled
  notes are typically ephemeral chat captures.
- **Ambiguous** (two notes share a normalized title): resolve to the
  **most-recently-updated** one.
- **Unresolved** (no matching note): render as a distinct "unresolved" link.
  Clicking it in the web editor offers to **create a note with that title**.
- **Storage is raw** — content keeps literal `[[Title]]`. No id rewriting.
  Resolution happens at index time (worker) and render time (SPA). This keeps
  chat and web identical and content portable, and makes "target created later"
  resolve for free.

Rejected: *unique-title enforcement* (annoying constraint, complicates chat
capture) and *`[[title|id]]` id-carrying links* (ugly, not chat-friendly).

## 4. Shared parser (single source of truth)

A new `grumps_core::wikilink` module — used by **both** the worker (indexing)
and the SPA (rendering) so parsing logic exists once and is unit-tested without
WASM.

```rust
pub struct Wikilink { pub target: String, pub alias: Option<String>, pub span: Range<usize> }

/// Extract all [[links]] from markdown, skipping fenced code blocks and inline
/// code spans. Returns links in source order.
pub fn extract_wikilinks(content: &str) -> Vec<Wikilink>;

/// Normalize a title/target for matching: trim, collapse whitespace, lowercase.
pub fn normalize_title(s: &str) -> String;
```

Parser requirements:
- Recognize `[[target]]` and `[[target|alias]]`.
- **Skip** matches inside fenced ```` ``` ```` blocks and inline `` `code` ``
  spans (so code samples containing `[[` aren't linkified).
- Ignore empty targets (`[[]]`).
- Deduplicate by normalized target for indexing (render keeps every occurrence).

## 5. Backlink index

**Chosen approach: a title-keyed edge table, resolved at query time.**

New migration `migrations/workspace/0010_note_links.sql` (version 10, `data:
false`), registered in `crates/worker/src/migrations.rs::workspace_migrations()`
and applied by the existing idempotent runner (`apply_pending`):

```sql
-- Precomputed normalized title on notes so joins are pure SQL equality
-- (SQLite can't call the Rust normalize()). Populated in Rust on write.
ALTER TABLE notes ADD COLUMN title_norm TEXT;
UPDATE notes SET title_norm = lower(trim(title)) WHERE title IS NOT NULL; -- best-effort backfill
CREATE INDEX IF NOT EXISTS idx_notes_title_norm ON notes(title_norm);

CREATE TABLE IF NOT EXISTS note_links (
  from_id       TEXT NOT NULL,
  to_title_norm TEXT NOT NULL,        -- normalized target title
  display       TEXT,                 -- alias or raw target, for outgoing render
  PRIMARY KEY (from_id, to_title_norm)
);
CREATE INDEX IF NOT EXISTS idx_note_links_to ON note_links(to_title_norm);
CREATE INDEX IF NOT EXISTS idx_note_links_from ON note_links(from_id);
```

**Why title-keyed, not id-keyed:** edges resolve to a note id by joining
`note_links.to_title_norm = notes.title_norm` at query time. A link to a
not-yet-created note simply starts matching the day that note is created — no
re-indexing pass, no stale ids on rename beyond the natural title-based
semantics.

**`title_norm` consistency:** it is always written by the **Rust**
`normalize_title()` in `insert_note`/`update_note` (the SQL `lower(trim())` above
is only a best-effort backfill for existing rows). Rust normalization is the
authority; the backfill is refreshed the next time each note is saved.

**Indexing (write path).** Done at the DB layer inside
`WorkspaceDb::insert_note` and `update_note`, so **every** creation path (web,
API, chat `NOTE:`) is covered uniformly:

0. Compute `title_norm = normalize_title(title)` and write it on the note row.
1. After the note row is written, `extract_wikilinks(content)`.
2. `DELETE FROM note_links WHERE from_id = ?`.
3. Insert one row per unique normalized target (with `display`).

`delete_note` also runs `DELETE FROM note_links WHERE from_id = ?`. (Incoming
edges to a deleted note simply stop resolving, since resolution is by title.)

**Query (read path).** One new endpoint powers both the backlinks panel and
resolving outgoing links to ids:

```
GET /api/w/:slug/notes/:id/links
→ {
    "backlinks": [ { "id", "title" }, ... ],   // notes linking TO this note
    "outgoing":  [ { "display", "id"|null } ]  // this note's links, id if resolved
  }
```

- `backlinks`: bind `:norm = normalize_title(this.title)` in Rust, then
  `SELECT n.id, n.title FROM note_links l JOIN notes n ON n.id = l.from_id
  WHERE l.to_title_norm = :norm`. (If `this.title` is null, backlinks are empty.)
- `outgoing`: `note_links` rows where `from_id = :id`. For each, resolve the id
  as `SELECT id FROM notes WHERE title_norm = l.to_title_norm ORDER BY
  updated_at DESC LIMIT 1` (most-recent wins on ambiguity; null = unresolved).

## 6. SPA changes

- **Wikilink rendering (read view).** A `wikilink` render helper walks note
  content via the shared parser and emits Leptos nodes: resolved links →
  `<a>` to `/w/:slug/notes/:id` (styled normal), unresolved → a distinct
  "unresolved" style whose click opens the create-note flow prefilled with the
  title. Non-link text passes through unchanged. **Full markdown rendering stays
  out of scope** — this transform only linkifies `[[…]]`; everything else renders
  as today. It slots into the future `pulldown-cmark` pipeline later.
  Link id resolution uses the `outgoing` array from the `/links` endpoint.
- **Autocomplete (editor).** On typing `[[`, open a dropdown of workspace note
  titles filtered by the partial query. Data source: reuse
  `GET /api/w/:slug/notes` (already returns id/title) filtered client-side —
  **no new endpoint**. Selecting inserts `[[Title]]` and closes the popup.
  Keyboard: ↑/↓ to move, Enter/Tab to accept, Esc to dismiss.
- **Backlinks panel.** Below the note read view, a "Linked from" section lists
  `backlinks` as navigable links. Hidden when empty.

## 7. i18n (required)

Per the project hard rule, no user-facing literals in source. New English keys
(then batch-translated to the other 13 locales):

- `notes.backlinks.heading` — "Linked from"
- `notes.backlinks.empty` — (panel hidden; no string needed)
- `notes.wikilink.unresolved_tooltip` — "This note doesn't exist yet"
- `notes.wikilink.create_prompt` — "Create note “{title}”?"
- `notes.editor.link_picker_empty` — "No matching notes"

## 8. Testing

- **Parser** (`grumps_core::wikilink`, pure, no WASM): extraction with/without
  alias; skips fenced + inline code; ignores empty; normalization
  (case/whitespace); dedup. The bulk of correctness lives here.
- **Indexing**: unit-test the edge-set produced for a given content string
  (rebuild on update replaces old edges; delete clears them).
- **Links endpoint**: backlink resolution incl. ambiguous title
  (most-recent wins) and unresolved outgoing (id null).
- **SPA**: render helper produces resolved vs unresolved nodes; autocomplete
  filters titles. (Component tests where the Leptos harness allows; otherwise
  cover the pure helpers.)

## 9. Out of scope (YAGNI)

- Folder/tree navigation and tags-as-hierarchy.
- Click-to-edit live preview / editor rework.
- Full markdown rendering pipeline (tracked separately under SPECS §9.4).
- Rendering wikilinks *inside chat messages* (bot stores raw `[[…]]`; notes are
  still indexed for backlinks). Can be added later without schema change.
- Graph view.

## 10. Accepted v1 limitations (implemented as-built)

These were surfaced during implementation review and consciously accepted for v1
(none block the core linking/backlinks value):

- **`[[` autocomplete completes the last open `[[` in the buffer** and, on
  select, replaces from that `[[` to the buffer end — so text typed *after* an
  open `[[` (when the caret isn't at the end) is discarded. Fine for the
  append-while-typing common case; a caret-index-precise version is future work.
- **`rebuild_note_links` is non-atomic** (DELETE then per-edge INSERT; the D1
  layer exposes no transaction primitive). A crash mid-rebuild leaves one note's
  outgoing edges stale/missing; it self-heals on that note's next save and never
  affects other notes or crashes reads.
- **`title_norm` backfill is best-effort.** The migration's SQL
  `lower(trim(title))` is ASCII-only and doesn't collapse inner whitespace, so it
  can differ from the Rust `normalize_title` authority for non-ASCII/multi-space
  titles. This can only *miss* a resolution (never mis-resolve), and each row is
  corrected on its next save.

The create-on-click flow for unresolved links (spec §3/§6) **is** implemented:
clicking an unresolved `[[Target]]` creates a note titled `Target` (seeded with a
`# Target` heading so it passes content validation) and navigates to its editor.

## 10. Isolation summary

| Unit | Purpose | Depends on |
|---|---|---|
| `grumps_core::wikilink` | Parse + normalize `[[links]]` | nothing (pure) |
| `note_links` table + index hook | Persist link edges on write | `wikilink`, `WorkspaceDb` |
| `GET …/notes/:id/links` | Serve backlinks + resolved outgoing | `note_links`, `notes` |
| SPA render helper | Linkify `[[…]]` in read view | `wikilink`, `/links` |
| SPA autocomplete | Title picker on `[[` | existing notes list API |
| SPA backlinks panel | "Linked from" list | `/links` |

Each unit is independently testable; the parser is the shared core everything
else builds on.
