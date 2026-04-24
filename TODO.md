# Grumps — TODO

## Now (this session)

- [ ] Setup Telegram bot (@BotFather → token → webhook → test in group)
- [ ] Deploy with Telegram support
- [ ] Test E2E: add bot to Telegram group → welcome message + description change
- [ ] Test E2E: TODO:/DONE:/NOTE: in Telegram group
- [ ] Test E2E: @grumps commands in Telegram group
- [ ] Test E2E: WhatsApp DM flow (send message to bot number)

## WhatsApp Groups (pending Meta approval)

- [ ] Apply for Meta Business Verification (https://business.facebook.com/settings/info)
- [ ] Apply for WhatsApp Groups API access (https://developers.facebook.com/docs/whatsapp/groups)
- [ ] Once approved: implement WhatsApp Groups adapter (create group, receive group messages)
- [ ] Auto-provision workspace on WhatsApp group creation
- [ ] Set WhatsApp group description with workspace link

## Infrastructure

- [ ] Create a proper Cloudflare API token (not OAuth) for long-term CF_API_TOKEN
- [ ] Setup a custom domain (grumps.app or similar)
- [ ] Deploy SPA to CF Pages (`trunk build --release` + `wrangler pages deploy`)
- [ ] Configure SPA API_BASE to point to production Worker URL
- [ ] Setup Stripe products (Pro €5/mo, Business €15/mo) + webhook
- [ ] Add `STRIPE_WEBHOOK_SECRET` to Worker secrets

## Code — Missing Features

- [ ] File upload via chat (`@grumps store` on media messages) — needs R2 bucket
- [ ] File upload via web (SPA drag & drop → Worker → R2)
- [ ] R2 bucket creation + signed URL generation
- [ ] Summarize command (`@grumps summarize` → LLM recap of recent chat)
- [ ] `@grumps quiet` toggle (check settings before responding)
- [ ] `@grumps lang FR` command to change workspace language
- [ ] Kanban view in SPA (drag & drop between columns)
- [ ] Dark mode toggle in SPA
- [ ] SPA polling (fetch every 30s on active page)
- [ ] Note editor: markdown preview with pulldown-cmark (WASM)

## Code — Improvements

- [ ] Worker: proper error responses (JSON format, not plain text)
- [ ] Worker: rate limiting via KV (per workspace, per IP)
- [ ] Worker: Stripe signature verification (currently accepts any payload)
- [ ] Worker: Ed25519 signature verification for Discord (currently stub)
- [ ] NLU: date parsing from natural language ("next friday" → ISO 8601)
- [ ] NLU: @mention → member_id resolution (currently stores raw @name)
- [ ] Handler: snooze/edit/reassign on task card replies (currently only updates status text, not DB)
- [ ] Handler: `@grumps list @Pierre` — resolve display_name to member_id for filter
- [ ] Cron: smarter recap scheduling (configurable per workspace, not just Mondays)
- [ ] DB: workspace name from first group message or chat title
- [ ] SPA: auth guard (redirect to /login if no JWT)
- [ ] SPA: optimistic updates on todo toggle
- [ ] SPA: real-time refresh after mutations (refetch after create/update/delete)

## Code — Testing

- [ ] Integration tests with mock D1 (test handler end-to-end without HTTP)
- [ ] Test the full parse → handle → format pipeline for each command
- [ ] SPA: component tests (if leptos testing tools mature)

## Marketing / Launch

- [ ] Landing page: deploy index.html to CF Pages or GitHub Pages
- [ ] Landing page: update "Add to WhatsApp" button to real link
- [ ] Landing page: add "Add to Telegram" button
- [ ] Privacy policy page
- [ ] Terms of service page
- [ ] Grumps.io domain setup (DNS → CF Pages for SPA, custom route for API)

## Done (reference)

- [x] Phase 1: Chat MVP (WhatsApp webhook, NLU, D1, task cards)
- [x] Phase 2: Web Workspace (Worker API + Leptos SPA)
- [x] Phase 3: LLM NLU, reminders, recaps, Stripe billing
- [x] Phase 4: Telegram/Discord adapters, recurring todos, export, i18n, PWA
- [x] Design system: workspace.html + index.html prototypes
- [x] Deployment guide: DEPLOY.md
- [x] Worker deployed to grumps-api.nodalstudio.workers.dev
- [x] D1 Index DB created + schema applied
- [x] KV namespace created
- [x] All secrets configured (WA, CF, JWT, Gemini, Anthropic)
- [x] E2E test: simulated webhook → workspace provisioned → todos created
