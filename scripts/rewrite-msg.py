#!/usr/bin/env python3
"""Second-pass cleanup after the initial Plan→generic rewrite.

Targets:
  - Awkward "foundation — Foundation (...)" doubled words left after the
    first pass (and the same for calendar / agent-loop / spa-pages / etc.).
  - Stray sub-step markers like "(E1 + E2)" or "(E3 + E4)" that referred
    to internal plan checklist items and carry no signal for a future reader.
"""
import re

msg = commit.message.decode('utf-8', errors='replace')

# 1. Collapse "<rewrite> — Word (...)" where Word matches the rewrite (case-insensitive).
#    Example: "docs: foundation — Foundation (data model, …)" → "docs: foundation (data model, …)"
def _dedup(m):
    rewrite = m.group(1)
    word = m.group(2)
    return rewrite if rewrite.lower() == word.lower() else f'{rewrite} \u2014 {word}'

msg = re.sub(
    r'\b(foundation|agent loop|calendar|web search|auto-extract & proactive|spa pages|deployment & runbook)\s+[\u2014\u2013\-]\s+([A-Z][\w \-]*?)\b',
    _dedup,
    msg,
)

# 2. Drop sub-step markers "(E1 + E2)", "(E3 + E4)", "(A1)", "(B12)", etc.
msg = re.sub(r'\s*\([A-G]\d+(?:\s*[+,]\s*[A-G]\d+)*\)', '', msg)

commit.message = msg.encode('utf-8')
