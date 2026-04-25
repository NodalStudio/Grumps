# Grumps — Feature opportunities from competitive analysis (OpenClaw)

**Date**: 2026-04-21
**Status**: Draft — competitive analysis + prioritized feature backlog
**Author**: Benoît Mayer (with Claude)
**Related**: [`2026-04-19-grumps-agent-design.md`](./2026-04-19-grumps-agent-design.md), [`SPECS.md`](../../../SPECS.md)

---

## 1. Context

OpenClaw (openclaw.ai) surfaced as a potential reference point while scoping Grumps. This doc records the comparison, maps the segment, and lists features worth stealing (or explicitly *not* stealing) to sharpen Grumps' positioning.

TL;DR: OpenClaw is **not a direct competitor**. It's a personal, local-first, OSS agent in the Poke/Jarvis lineage. Grumps is a **group** workspace bolted onto chat. Different users, different business model, different moat. But OpenClaw ships a few patterns that slot cleanly into Grumps and would noticeably lift the product without diluting the focus.

---

## 2. What OpenClaw is

Open-source AI assistant that runs locally and automates tasks through chat interfaces.

- **User model**: 1 user → 1 personal agent
- **Hosting**: local or self-hosted (Mac/Windows/Linux)
- **Chat surfaces**: WhatsApp, Telegram, Discord, Slack, Signal, iMessage
- **Models**: Claude, GPT, local (MiniMax 2.5) — user's choice
- **Capabilities**: email management, calendar, flight check-ins, shell exec, file read/write, web browsing, form-filling, cron, self-written skills
- **Integrations**: Gmail, Calendar, Spotify, Hue, Obsidian, Twitter, GitHub, 1Password, Railway, 50+ more
- **Business**: free / open-source
- **Target**: power users who want a personal OS-level assistant

Positioning is basically *"Poke, but open-source and self-hosted"*.

---

## 3. Axis-by-axis comparison

| Axis | Grumps | OpenClaw |
|---|---|---|
| User model | **Group** (N users → 1 shared bot) | **Personal** (1 user → 1 agent) |
| Hosting | Cloudflare SaaS, managed | Local-first / self-hosted |
| Scope | Workspace chat (todos/notes/files/reminders + web) | OS-level, general-purpose |
| LLMs | Gemini Flash + Haiku (fixed cascade) | Claude / GPT / local (user picks) |
| Integrations | Messaging only (WA/TG/Discord/Slack) | Gmail, Calendar, Spotify, Hue, Obsidian, GitHub, 1Password, Railway, +50 |
| Agentic depth | Regex → LLM → D1 CRUD | Shell exec, file I/O, browser automation, cron, self-written skills |
| Business | Freemium €5 / €15 per workspace | Free / OSS |
| Target | Roommates, small teams, family/hobby groups | Technical individuals |
| Moat | Web workspace + group focus | Skill ecosystem + self-hosting |

**Conclusion**: a solo power user who wants to automate their life picks OpenClaw. A 5-person roommate household that wants a shared shopping list picks Grumps. They are adjacent, not substitutable.

---

## 4. Competitive map

```
                       PERSONAL                   GROUP
                   ┌─────────────────────┬──────────────────────┐
       CHAT-ONLY   │  Poke (SaaS)        │  TaskRio             │
                   │  Dola AI            │  Any.do WA bot       │
                   │  WhatsApp Meta AI   │  Slack/Discord bots  │
                   │                     │                      │
                   ├─────────────────────┼──────────────────────┤
       CHAT + WEB  │  OpenClaw (OSS)     │  ★ GRUMPS ★          │
                   │  ChatGPT + apps     │  (mostly empty)      │
                   │  Claude Projects    │                      │
                   │                     │                      │
                   ├─────────────────────┼──────────────────────┤
     WEB-FIRST     │  Notion AI          │  Notion / Linear     │
     (chat bolt-on)│  Obsidian           │  Slack canvas        │
                   └─────────────────────┴──────────────────────┘
```

Key observations:

1. **The group + chat + web quadrant is nearly empty.** TaskRio is the closest match and has no web workspace. This is the real moat — defend it with care.
2. **The serious threat is Meta itself.** WhatsApp is integrating Meta AI natively. The day they ship "shared group tasks", a single-platform product suffers. Mitigations already in SPECS: multi-platform adapter (TG/Discord/Slack, not just WA) + rich web workspace that Meta will not build.
3. **Poke lives in the same UX family** (chat → actions) but for a solo user on SMS/WA. If Poke pivots to group, it becomes a frontal competitor. Watch.
4. **OpenClaw is an opportunity, not a threat.** Co-marketing line: *"For your personal life, use OpenClaw/Poke. For your groups, use Grumps."*

---

## 5. Features worth stealing — quick wins (≤ 2 weeks each)

These fit the current Grumps model without dragging it toward "general agent". Rough effort estimates assume the Phase 1+2 plumbing is in place.

| # | Feature | Why it fits | Rough effort |
|---|---|---|---|
| 1 | **AI auto-summary of uploaded files** | PDF/image uploaded → Gemini generates a summary stored as a linked note. High wow-factor for trivial cost. Uses the R2 + notes pipeline already in SPECS. | ~3 days |
| 2 | **Google Calendar 2-way sync** | Todos with deadlines ↔ shared calendar events. Kills the main reason people still open Google Cal next to WhatsApp. Massive for roommates/teams. | ~1 week |
| 3 | **Email → todo (inbound forwarding)** | Each workspace gets an address like `x7k9m2p4@in.grumps.io`. Forward an email, it becomes a todo + note. Low-cost, very expected. | ~3-4 days |
| 4 | **Group memory / facts** | `@grumps remember Bob is vegetarian` / `@grumps what does Bob not eat?`. Simple KV table, injected into the system prompt for recaps and NLU. Both OpenClaw and Poke lean heavily on memory. | ~2-3 days |
| 5 | **User-defined scheduled automations** | Cron Trigger plumbing is already in place for reminders/recaps. Expose it: `@grumps every friday 6pm ask what we're eating tonight`. Your cron, but outputting a question into the chat. | ~1 week |
| 6 | **Polls / interactive messages** | `@grumps poll "pizza or sushi?" pizza sushi indian` → WhatsApp interactive reply buttons. Native feature requested in any group chat. Meta supports it. | ~3-4 days |
| 7 | **Web clipper (URL → note)** | Paste a URL to the bot → fetch, extract, summarize, store as a note. Lightweight, leverages the existing note pipeline. | ~2 days |
| 8 | **Workspace-admin model picker** | Simple toggle: Gemini Flash (default) / Claude Haiku (smarter, part of Pro). Gets a marketing line ("powered by Claude") and an upsell lever into Pro. | ~1 day |

These eight features sharpen the "**workspace in chat**" positioning rather than pulling toward general-purpose agentism.

---

## 6. Features worth considering — larger chantiers

Valuable but expensive. Shortlist for post-MVP.

- **User-defined skills / plugins** (OpenClaw/Poke style) — very powerful but huge complexity (sandboxing, security, UX). Not MVP.
- **Agentic web automation** (*"@grumps book a table at Dupont on Friday"*) — frontal competition with OpenClaw / Poke / ChatGPT Agent. Probably not Grumps' angle; revisit only if demanded.
- **GitHub / Linear / Jira integration** — relevant for dev-team groups, but a narrow segment for Grumps. Wait for demand signal.
- **Voice note → transcription + action** — WhatsApp pushes voice notes hard, so this is a natural input. Whisper is cheap. Medium effort (~2 weeks).

---

## 7. Features NOT to copy — and why

Explicit no-go list, kept here so future discussions don't relitigate.

- **Self-hosting / local mode** — kills the business model, and the target audience (families, roommates, hobby groups) does not want to install a binary.
- **Arbitrary shell execution / file write** — unmanageable in a group context. Who authorizes what? No clean UX.
- **50 integrations up front** — focus is the moat against Poke/OpenClaw, which fan out. Integrate only what groups actually share.
- **Self-writing skills** — dangerous in multi-tenant, and irrelevant for the target audience.

---

## 8. Strategic implication

Grumps' durable edge is **focus**: group + chat + web workspace, done well. Everything above is evaluated against "does this strengthen the focus, or dilute it?"

- Features 1-8 in §5: *strengthen*. Ship them.
- §6 items: *dilute unless demanded*. Hold.
- §7 items: *dilute*. Never.

The competitive threat vector to actively watch is not OpenClaw — it's **Meta shipping native group AI in WhatsApp**. The mitigation already exists in the SPECS (multi-platform from day 1, web workspace as the defensible layer). Keep executing on both.

---

## 9. Suggested sequencing

Proposed rollout over the ~3 weeks following Phase 2 completion:

1. **Week 1**: #1 (file summary), #4 (group memory), #8 (model picker)
2. **Week 2**: #3 (email → todo), #7 (URL clipper)
3. **Week 3**: #6 (polls), #5 (user-defined automations)
4. **Later**: #2 (Google Calendar) — deserves its own spec doc given OAuth scope.

Each lands independently; none blocks another.
