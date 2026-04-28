---
allowed-tools: Bash(git add:*), Bash(git commit:*), Bash(git status:*), Bash(git diff:*), Bash(git log:*)
description: Review staged changes and create atomic conventional commits
model: haiku
disable-model-invocation: true
---

# I'll review staged changes, then create commits using the conventional commits specification (Angular standard), grouping changes by functional domain (prefer fewer, broader commits over many granular ones)

## Phase 1 — Quick Review

Before committing, review the staged diff (`git diff --cached`) against this checklist. Only flag issues that appear **in the diff itself** (changed/added lines):

**Security**
- Missing `@require_permission()` on new view functions
- `raw()` SQL queries, `mark_safe()` on user-controlled content, `@csrf_exempt`
- Secrets, credentials, or API keys hardcoded in code

**Architecture**
- New models not inheriting `BaseModel` or missing `AuditMixin`
- Class-based views (CBV) instead of function-based views (FBV)
- POST/PATCH/PUT handlers missing `updated_by = request.user`

**Performance**
- Querysets traversing FK/M2M without `select_related()` / `prefetch_related()`
- Obvious N+1 query patterns (queries inside loops)

**Quality**
- New functions/methods missing type hints on signatures
- French/English convention violations (English for code, French for UI text)

**Convention**
- Forms missing `apply_daisyui_styling()` call in `__init__`
- Partial templates not prefixed with `_` or not inside a `partials/` directory

**If issues are found**, output them and **stop** (do not commit):

```
⚠ Review found issues — commits blocked:

1. [Security] apps/foo/views.py:42 — missing @require_permission()
   → Fix: add @require_permission('foo.view') before the view function

2. [Convention] apps/foo/forms.py:15 — missing apply_daisyui_styling()
   → Fix: add apply_daisyui_styling(self) call in __init__
```

**If no issues**, proceed to Phase 2.

## Phase 2 — Conventional Commits

**Format:** `type(scope): subject` (lowercase, no period, imperative mood)

**Examples:**

- `feat(auth): add two-factor authentication`
- `fix(api): resolve null pointer in user endpoint`
- `docs: update installation guide`
- `refactor(reporting): simplify calculation logic`

**Commit Types:**

**feat** - A new feature for the user
**fix** - A bug fix
**docs** - Documentation only changes
**style** - Changes that do not affect the meaning of the code (whitespace, formatting)
**refactor** - Code change that neither fixes a bug nor adds a feature
**perf** - Performance improvement
**test** - Adding or correcting tests
**build** - Changes to build system or external dependencies
**ci** - Changes to CI configuration files and scripts
**chore** - Tooling, AI pipeline and DX improvements
**revert** - Reverts a previous commit

**Scope Guidelines:**

- Optional but recommended
- Identifies the affected module/package/feature
- Use lowercase
- Examples: `auth`, `api`, `ui`, `database`, `reporting`
- Omit for cross-cutting changes or docs updates

**Subject Requirements:**

- Use imperative mood ("add" not "added" or "adds")
- Don't capitalize first letter
- No period at the end
- Maximum 100 characters (full line including type and scope)
- Be specific and descriptive

**Body (Optional):**

- Separate from subject with blank line
- Explain the _why_ behind changes, not the _what_
- Use imperative mood
- Can include multiple paragraphs
- Wrap at 72 characters

**Footer (Optional):**

- Reference issues: `Fixes #123` or `Closes #456`
- Breaking changes: `BREAKING CHANGE: description`
- Deprecations: `DEPRECATED: description`

**Breaking Changes:**

``` text
feat(api)!: remove deprecated v1 endpoints

BREAKING CHANGE: v1 API endpoints have been removed.

Migrate to v2 endpoints by updating base URL from /api/v1 to /api/v2
```

**Multi-paragraph Body Example:**

``` text
fix(auth): prevent session hijacking vulnerability

The previous implementation stored session tokens in localStorage
which was vulnerable to XSS attacks.

Session tokens are now stored in httpOnly cookies with secure
and sameSite flags enabled.
```

**Strategy - Functional Grouping (prefer fewer, broader commits):**

- **Group by functional domain**, not by file type or technical layer
- A single feature/refactoring touching models, views, forms, templates, and tests = ONE commit
- Examples of good groupings:
  - All changes to the `controls` app (models + views + forms + templates + services) = 1 commit
  - All UI/frontend changes (CSS + JS + layout templates) = 1 commit
  - All infrastructure changes (settings + docker + deps) = 1 commit
- **Avoid splitting** related changes across multiple commits just because they touch different file types
- Only split into separate commits when changes are **truly independent features**
- Use the commit body to list the key changes when a commit is broad
- Write meaningful commit messages that explain the "why"
- Do not add Claude Code attribution to commit messages

**Typical commit count for a feature:**

- Small bug fix: 1 commit
- New feature in one app: 1-2 commits
- Major refactoring: 2-4 commits (by functional domain)
- Cross-cutting changes: group by independence, not by file type

Let me review your staged changes and then create functional commits.
