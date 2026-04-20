# Grumps — Working notes for Claude

## Design system

Source of truth: `docs/design-system.md`. All frontend work
(`crates/spa/`, `*.html`, i18n strings) must conform to it.
Key rules:
- No emoji in UI chrome — use `<Icon name="..."/>`.
- Use semantic tokens (`--surface-base`, `--text-primary`, …) in new
  code so it works in both light and dark mode.
- Named typography scale (`display-xl/lg`, `display`, `body`,
  `body-sm`, `meta`, `eyebrow`) — no `text-[Npx]` in source.
- 2px borders on strong containers; `rounded-sm` (= 3px) everywhere.
- Offset-block shadow reserved for hero cards, 1–2 per page max.
- Terse voice: no "!", no emoji, no "please". The landing may relax
  slightly for warmth but keeps the punctuation and emoji rules.
