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
