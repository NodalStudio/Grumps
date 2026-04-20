# Grumps — Working notes for Claude

## Project shape

Group-chat AI agent (WhatsApp / Telegram / Discord) with a workspace
SPA. Rust+WASM on Cloudflare Workers, D1 + KV + Vectorize. Anthropic
Sonnet + Gemini Flash cascade. See `SPECS.md` for the full spec and
`docs/superpowers/` for designs and historical plans.

## Internationalization (REQUIRED)

Grumps ships in **14 languages**:

`en` · `es` · `pt-BR` · `fr` · `de` · `it` · `ru` · `tr` · `ar` (RTL) · `hi` · `zh-CN` · `ja` · `ko` · `id`

**Hard rule: no user-facing text in source code.** Every string visible
to a user — landing page, SPA, bot messages, error toasts, email
templates, OG tags, push notifications — MUST go through the i18n layer.
If you find yourself typing a sentence directly into a `.rs`, `.html`,
or template file, stop and add a key to the appropriate dictionary
instead.

### Where strings live

A single `crates/i18n/` crate is the source of truth for both worker
and SPA. Flat-key JSON dictionaries (no Fluent) — keeps the SPA WASM
bundle small and the API trivial.

| Surface | Format | Source files |
|---|---|---|
| Landing page | JSON | `landing/strings.{lang}.json` |
| SPA + bot/worker | JSON via `grumps-i18n` | `crates/i18n/locales/{lang}.json` |
| LLM-generated text | n/a — system prompt injects the target locale; the LLM replies in the user's locale |

### When adding a feature

1. Add the English key first to the appropriate JSON.
2. Reference it via `t(locale, "key", &[("var", "value")])` (worker)
   or `tr(key)` / `tr_p(key, params)` / `tr_n(key_base, n, params)` (SPA helpers).
   For the landing, use `data-i18n="key"` on the element.
3. Run `scripts/i18n-translate.sh KEY` (TODO: build) to fill the 13
   other locales via Sonnet batch. Manually review `fr` and `es`;
   ship the rest as-is.
4. Test `ar` visually for RTL layout.
5. Test `tr` visually — Turkish phrases run ~15-20% longer than EN and
   often expose tight `width:` constraints. Prefer `min-width` over
   fixed widths on buttons, tabs, badges.

### Pluralization

**Never use `"{n} message(s)"` or `if n == 1 { "" } else { "s" }`** —
it works for English only and reads as broken in every other language.

Use the `t_plural()` / `tr_n()` helper which picks `key_base.{plural-category}`
based on the locale's CLDR rule. Plural categories supported per locale:

| Locale family | Categories |
|---|---|
| en, de, es, pt-BR, it, tr, hi, id | `one` (n=1), `other` |
| fr | `one` (n=0 or 1), `other` |
| ru | `one`, `few` (2-4), `many` (5+) |
| ar | `zero`, `one`, `two`, `few` (3-10), `many` (11-99), `other` |
| zh-CN, ja, ko | `other` (no plural distinction) |

Example dictionary entries:

```json
"schedule.fired.one":   "Fired {n} time",
"schedule.fired.other": "Fired {n} times",
"schedule.fired.few":   "Запущено {n} раза",
"schedule.fired.many":  "Запущено {n} раз"
```

`tr_n("schedule.fired", n, &[])` automatically picks the right key and
injects `{n}`. The `t_plural()` fallback chain is per-category-key →
`.other` → English → literal key, so missing translations degrade
gracefully but visibly.

### Locale detection

- **Landing**: `Accept-Language` → CF Pages redirect → `/{lang}/`
  (fallback `en`). Lang switcher in footer.
- **SPA**: `?lang=` query > `localStorage.locale` > `Accept-Language` > `en`
- **Bot**: per-member `members.locale` column. Set on first message
  from the language detected by the Gemini classifier (free win — it
  already returns a language hint). Group-level default fallback, then
  `en`.

### RTL

Only `ar` is RTL. SPA: layout uses logical CSS properties
(`margin-inline-*`, `padding-inline-*`, `inset-inline-*`) where
possible. Landing: ships a small RTL stylesheet variant loaded
conditionally.

### CJK + Devanagari fonts

`Bitter` (display) and `DM Sans` (body) don't cover Chinese, Japanese,
Korean, Hindi, or Arabic glyphs. Each non-Latin locale loads a Noto
fallback in its `<head>`:
- `zh-CN`: Noto Sans SC + Noto Serif SC
- `ja`: Noto Sans JP + Noto Serif JP
- `ko`: Noto Sans KR + Noto Serif KR
- `hi`: Noto Sans Devanagari + Noto Serif Devanagari
- `ar`: Noto Sans Arabic + Noto Serif Arabic

The build script (`landing/build.mjs`) picks the right `<link>` per
locale variant.

## Demo mode

The landing page embeds the actual SPA in an iframe rather than a
hand-maintained HTML mockup. When the SPA is loaded under `/demo/...`
or with `?demo=1`, every API call is short-circuited to deterministic
seed data and the auth gate is bypassed. See `crates/spa/src/demo.rs`.

Build the SPA bundle into `crates/spa/dist/` with:

```bash
cd crates/spa
mkdir -p dist
tailwindcss -i ./input.css -o ./dist/styles.css --minify
MSYS_NO_PATHCONV=1 trunk build --release --public-url /demo/
```

The landing build (`node landing/build.mjs`) detects `crates/spa/dist/`
and copies it into `dist/demo/`, then rewrites the iframe src to
`/demo/?lang={code}` so the iframe locale matches the surrounding
landing.

## Design system

Warm brutalism. Slab serif (`Bitter`) for headings, `DM Sans` for body.
Cream `#F5F0E8` base + brick `#C0392B` for urgency, teal `#1B6B5A` for
success, ochre `#D4940A` for warnings. 2px ink borders. No shadows
beyond the offset-block style. Subtle grain overlay (SVG noise).
Voice: terse and dry — "Done." not "Successfully completed!" — no
emojis in UI chrome. Reference: `index.html` and `crates/spa/`.

## Git workflow

Linear history only. Always rebase, never merge. PRs land via
"Rebase and merge" on GitHub. Commit messages MUST NOT include
`Co-Authored-By:` trailers and MUST NOT reference internal plan
identifiers (e.g. "Plan A", "Plan B"); use generic descriptive
language instead.

## Rust toolchain

Tests: build with `--target x86_64-pc-windows-msvc`. Use rustup's
`cargo` (`~/.cargo/bin/cargo.exe`) — do not pick up the Chocolatey
shim. Recommended invocation:

```bash
PATH="/c/Users/mayer/.cargo/bin:$PATH" ~/.cargo/bin/cargo.exe test \
  --target x86_64-pc-windows-msvc
```

## Admin model

Two tiers:
- **Workspace admin** — manages a single group's settings and members.
- **Super admin** — Benoît only. Identified by phone number in the
  `SUPER_ADMIN_PHONES` env var. Sole access to the cross-workspace
  observability dashboard, cost analytics, and global admin tools.

Never expose super-admin surfaces to workspace admins — they're
distinct roles.
