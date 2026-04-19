# Grumps — Évolution vers un agent IA persistant pour groupes

**Date** : 2026-04-19
**Statut** : Spec validé (brainstorming), en attente de plan d'implémentation
**Auteur** : Benoît Mayer (avec Claude)
**Spec lié** : [`2026-04-13-grumps-design.md`](./2026-04-13-grumps-design.md) — design v4 actuel

---

## 1. Vision

Faire évoluer Grumps d'un "bot todos/notes léger" vers un **véritable agent IA persistant pour groupes de messagerie**, dans l'esprit de Claude (mémoire continue, raisonnement, capacité à utiliser des outils, planification d'actions futures), mais ancré dans un usage **collectif** : Grumps sert le groupe, pas l'individu.

L'agent retient ce qui a été dit, peut planifier des actions futures (rappels, suivis, recaps, tâches autonomes), et expose un calendrier centralisé qui agrège tâches, événements et rappels.

### Tagline étendue
> *"Gets it done. No small talk. Remembers everything that matters."*

### Positionnement vs concurrence
- **TaskRio / WhatsTask** : todos en chat. Grumps = todos + mémoire + agent + calendrier + workspace web.
- **ChatGPT / Claude.ai** : assistants individuels. Grumps = assistant **collectif et persistant** ancré dans un canal de discussion existant.

---

## 2. Scope

### In scope (V1)
- Mémoire structurée explicite par workspace (mode 1)
- RAG sémantique sur l'historique du chat (mode 3)
- Auto-extraction de mémoire depuis les messages, **opt-in par workspace** (mode 2)
- Mode proactif **opt-in par workspace** (l'agent intervient sans @mention) — réservé Pro+
- Boucle agent avec tool use (Claude Sonnet 4.6) : query mémoire, RAG, CRUD todos/notes/events/reminders, web search, schedule action
- Actions planifiées : rappels passifs, suivis conditionnels, triggers événementiels, tâches agentiques différées
- Calendrier workspace agrégeant todos+reminders+events+scheduled actions, avec drag & drop et **export iCal** (lecture seule pour Google/Apple/Outlook)
- Web search via Brave Base (provider abstraction permet SearXNG en alternative)
- Pages SPA : Calendrier, Mémoire, Actions planifiées
- Quotas par plan (Free/Pro/Business)

### Out of scope (V1, peut être V2+)
- Sync bidirectionnel Google Calendar / Outlook
- DM bot pour usage personnel
- Notifications push PWA / email digest
- Tools agentiques étendus : email envoyé, réservations transactionnelles (OpenTable, Doctolib)
- Marketplace de tools tiers

### Hors scope définitivement
- Mémoire transverse aux workspaces (chaque workspace = silo étanche)
- Sortie du périmètre Cloudflare (sauf provider web search OSS auto-hébergé optionnel)

---

## 3. Décisions de cadrage (résultat brainstorming)

| Question | Décision |
|---|---|
| Niveau de proactivité | **Configurable par workspace**. Défaut : semi-proactif (b) — exécution automatique des actions schedulées, pas d'intervention spontanée dans la conversation. Mode (c) proactif disponible en opt-in. |
| Scope mémoire | **(1) structurée + (3) RAG** activés par défaut. **(2) auto-extraction** disponible dès le lancement, opt-in par workspace avec consentement explicite. Orientée grand public (famille, amis, coloc, hobbies, pas que pro). |
| Types d'actions schedulées | **A** rappels passifs, **B** suivis conditionnels, **C** triggers événementiels, **E** actions agentiques externes limitées à **web search** au lancement. **D** (préparation contextuelle) émerge gratuitement de la combinaison C + RAG. |
| Calendrier | **1 calendrier par workspace** (filtrable par membre dans la vue). Sync **iCal sortant** vers Google/Outlook/Apple, **pas de sync bidirectionnel**. UI : vues mois + semaine + agenda mobile, drag & drop pour replanifier. |
| Notifications | **Uniquement chat de groupe**. Pas de DM bot, pas de push PWA, pas d'email — Grumps est un outil collectif du groupe pour le groupe. |
| Stratégie produit | **Vision complète développée d'un coup** (pas de phasage MVP). |

---

## 4. Architecture d'ensemble

### Principe directeur

100% Cloudflare serverless préservé. On ajoute une couche "agent" par-dessus l'existant **sans casser le bot actuel** :
- Les commandes structurées (`TODO:`, `DONE:`, replies aux task cards) continuent à passer par le fast-path regex sans toucher à Sonnet.
- L'agent (Sonnet) n'est invoqué que quand le contexte le justifie.

### Schéma logique

```
                     WhatsApp / Telegram / Discord
                              │  webhook
                              ▼
        ┌──────────────────────────────────────────────────┐
        │              CF Worker (entrypoint)              │
        │                                                  │
        │  ┌────────────┐                                  │
        │  │  Webhook   │── parse + verify signature       │
        │  └─────┬──────┘                                  │
        │        ▼                                         │
        │  ┌──────────────────────────────────┐            │
        │  │       Message Router             │            │
        │  │  fast-path (regex)               │ ──→ legacy │
        │  │  vs. classifier vs agent loop    │ ──→ agent  │
        │  └─────────────┬────────────────────┘            │
        │                ▼                                 │
        │   ┌──────────────────────────────────────┐       │
        │   │      Gemini Flash classifier         │       │
        │   │  intent + confidence + args          │       │
        │   └────────┬───────────────────┬─────────┘       │
        │            │ simple            │ complex/conf<.85│
        │            ▼                   ▼                 │
        │     direct CRUD       ┌──────────────────────┐   │
        │     (zero LLM)        │  AGENT LOOP (Sonnet) │   │
        │                       │ ┌────────────────┐   │   │
        │                       │ │ tools registry │   │   │
        │                       │ │ - query_memory │   │   │
        │                       │ │ - query_chat_h │   │   │
        │                       │ │ - create_*     │   │   │
        │                       │ │ - schedule_act │   │   │
        │                       │ │ - web_search   │   │   │
        │                       │ │ - save_memory  │   │   │
        │                       │ └────────────────┘   │   │
        │                       └──────────┬───────────┘   │
        │                                  ▼               │
        │                       response → group chat      │
        │                                                  │
        │  ┌────────────────────────────────────────┐      │
        │  │  Auto-extract pipeline (opt-in mode 2) │      │
        │  │  every msg → Gemini Flash → maybe save │      │
        │  └────────────────────────────────────────┘      │
        │  ┌────────────────────────────────────────┐      │
        │  │  Proactive classifier (opt-in mode c)  │      │
        │  │  every msg → Gemini → maybe Sonnet →   │      │
        │  │  maybe response                        │      │
        │  └────────────────────────────────────────┘      │
        └──────┬─────────────────┬─────────────────┬───────┘
               │                 │                 │
               ▼                 ▼                 ▼
        ┌────────────┐    ┌────────────┐   ┌────────────┐
        │ D1 (per-ws)│    │ Vectorize  │   │ Workers AI │
        │ + memory   │    │ chat-RAG   │   │ embeddings │
        │ + events   │    │ index      │   │ bge-m3     │
        │ + sched_act│    └────────────┘   └────────────┘
        │ + sessions │
        └─────┬──────┘
              │
              │ RPC schedule(action_id, trigger_at)
              ▼
        ┌──────────────────────────────────────────┐
        │  Durable Object : WorkspaceScheduler     │
        │  one instance per workspace_slug         │
        │  - holds setAlarm(next_trigger_at)       │
        │  - alarm() handler → execute due actions │
        └──────────────────────────────────────────┘

        ┌──────────────────────────────────┐
        │ iCal endpoint /cal/:slug.ics     │ ←── Google/Outlook subscribe
        └──────────────────────────────────┘
```

### Crates Rust (workspace étendu)

```
grumps/
├── Cargo.toml                  (workspace)
├── crates/
│   ├── core/                   (existant — types domaine)
│   ├── nlu/                    (existant — regex parsing)
│   ├── messaging/              (existant — adapters WA/TG/Discord)
│   ├── worker/                 (existant — étendu avec routes nouvelles)
│   ├── spa/                    (existant — étendu avec pages nouvelles)
│   ├── agent/                  ★ nouveau
│   │   └── src/
│   │       ├── router.rs       (Gemini classifier + dispatch)
│   │       ├── loop.rs         (boucle Sonnet + tool dispatch)
│   │       ├── prompt.rs       (system prompt builder)
│   │       ├── session.rs      (CRUD agent_sessions)
│   │       ├── proactive.rs    (mode c : classifier + filter Sonnet)
│   │       └── tools/
│   │           ├── mod.rs      (registry)
│   │           ├── memory.rs
│   │           ├── rag.rs
│   │           ├── todos.rs
│   │           ├── notes.rs
│   │           ├── calendar.rs
│   │           ├── scheduler.rs
│   │           └── web.rs      (provider abstraction Brave/SearXNG)
│   ├── memory/                 ★ nouveau
│   │   └── src/
│   │       ├── entries.rs      (CRUD memory_entries + FTS)
│   │       ├── auto_extract.rs (pipeline classifier)
│   │       ├── rag.rs          (Vectorize ingest + query)
│   │       └── consent.rs      (opt-in/opt-out flow)
│   ├── scheduler/              ★ nouveau
│   │   └── src/
│   │       ├── actions.rs      (CRUD scheduled_actions)
│   │       ├── condition.rs    (evaluator des 5-6 types)
│   │       ├── executor.rs     (dispatch par action_type)
│   │       ├── recurrence.rs   (RRULE parser/computer)
│   │       └── workspace_scheduler.rs  (Durable Object)
│   └── calendar/               ★ nouveau
│       └── src/
│           ├── events.rs       (CRUD events)
│           ├── aggregate.rs    (union 4 sources)
│           ├── recurrence.rs   (réutilise scheduler::recurrence)
│           ├── ical.rs         (génération VCALENDAR)
│           └── ical_token.rs   (JWT issuance/revoke)
```

### Couches & responsabilités

| Couche | Responsabilité |
|---|---|
| **Adapters** (`messaging`) | WhatsApp/Telegram/Discord → `InboundMessage` unifié |
| **Router** (`agent::router`) | fast-path regex → classifier Gemini → dispatch CRUD direct ou agent loop |
| **Agent** (`agent::loop`) | Boucle Sonnet, tool use, sessions multi-turn |
| **Memory** (`memory`) | CRUD `memory_entries` + RAG Vectorize + auto-extract pipeline |
| **Scheduler** (`scheduler`) | CRUD `scheduled_actions` + DO `WorkspaceScheduler` + condition evaluator + executor |
| **Calendar** (`calendar`) | CRUD `events` + aggregation view + iCal generator |
| **SPA** (`spa`) | Nouvelles pages : Calendar, Memory, Scheduled Actions |

### Décisions clés

1. **Fast-path préservé** : `TODO:` etc. ne touchent pas Sonnet → coût inchangé sur les usages structurés
2. **Routing en cascade** : classifier Gemini Flash en amont (~$0.0002/msg) → CRUD direct si confiance > 0.85 et intent simple, sinon escalation Sonnet
3. **Agent loop bornée** : max 5 tool turns par invocation (sécurité coût + latence)
4. **Conversation multi-turn** : table `agent_sessions` (1h TTL) permet à l'agent de répondre à des suites comme "oui crée-le", "non plutôt mardi"
5. **Tout reste workspace-scoped** : aucune donnée transverse entre groupes (silos étanches, garantie sécurité + RGPD)
6. **Scheduling sans polling** : Durable Objects avec alarmes natives CF, zéro cron continuel
7. **Provider web search abstrait** : Brave Base par défaut (commercial, $3/CPM), SearXNG en option (OSS, self-hosted)

---

## 5. Data model

Toutes les nouvelles tables sont **par-workspace** (dans la D1 du groupe), sauf indication contraire. Aucun ajout dans l'Index DB partagé.

### 5.1 `memory_entries` — mémoire structurée

```sql
CREATE TABLE memory_entries (
    id              TEXT PRIMARY KEY,           -- UUID
    key             TEXT,                       -- nullable, courte étiquette ("wifi-bureau")
    value           TEXT NOT NULL,              -- contenu libre, markdown
    kind            TEXT NOT NULL,              -- 'fact' | 'person' | 'decision' | 'preference' | 'place' | 'other'
    related_member  TEXT REFERENCES members(id),-- nullable, si mémoire concerne quelqu'un
    tags            TEXT DEFAULT '[]',          -- JSON array
    source          TEXT NOT NULL,              -- 'chat-explicit' | 'chat-auto' | 'web' | 'agent'
    confidence      REAL DEFAULT 1.0,           -- 0..1, < 1 si auto-extraite
    pinned          INTEGER DEFAULT 0,          -- 1 = toujours dans le contexte agent
    expires_at      TEXT,                       -- nullable
    created_by      TEXT REFERENCES members(id),
    created_at      TEXT DEFAULT (datetime('now')),
    updated_at      TEXT DEFAULT (datetime('now'))
);
CREATE INDEX idx_mem_kind ON memory_entries(kind);
CREATE INDEX idx_mem_pinned ON memory_entries(pinned) WHERE pinned = 1;
CREATE INDEX idx_mem_member ON memory_entries(related_member);
CREATE INDEX idx_mem_expires ON memory_entries(expires_at) WHERE expires_at IS NOT NULL;

CREATE VIRTUAL TABLE memory_fts USING fts5(key, value, content=memory_entries, content_rowid=rowid);

CREATE TRIGGER memory_ai AFTER INSERT ON memory_entries BEGIN
    INSERT INTO memory_fts(rowid, key, value) VALUES (new.rowid, new.key, new.value);
END;
CREATE TRIGGER memory_ad AFTER DELETE ON memory_entries BEGIN
    INSERT INTO memory_fts(memory_fts, rowid, key, value) VALUES ('delete', old.rowid, old.key, old.value);
END;
CREATE TRIGGER memory_au AFTER UPDATE ON memory_entries BEGIN
    INSERT INTO memory_fts(memory_fts, rowid, key, value) VALUES ('delete', old.rowid, old.key, old.value);
    INSERT INTO memory_fts(rowid, key, value) VALUES (new.rowid, new.key, new.value);
END;
```

### 5.2 `events` — événements calendrier

```sql
CREATE TABLE events (
    id              TEXT PRIMARY KEY,
    title           TEXT NOT NULL,
    description     TEXT,
    starts_at       TEXT NOT NULL,              -- ISO 8601 avec TZ
    ends_at         TEXT,                       -- nullable
    all_day         INTEGER DEFAULT 0,
    location        TEXT,
    recurrence      TEXT,                       -- RFC 5545 RRULE
    attendees       TEXT DEFAULT '[]',          -- JSON array of member.id
    color           TEXT DEFAULT 'teal',
    source          TEXT DEFAULT 'web',         -- 'chat' | 'web' | 'agent'
    related_todo_id TEXT REFERENCES todos(id),  -- nullable
    created_by      TEXT REFERENCES members(id),
    created_at      TEXT DEFAULT (datetime('now')),
    updated_at      TEXT DEFAULT (datetime('now'))
);
CREATE INDEX idx_events_starts ON events(starts_at);
CREATE INDEX idx_events_recur ON events(recurrence) WHERE recurrence IS NOT NULL;
```

### 5.3 `scheduled_actions` — actions planifiées

```sql
CREATE TABLE scheduled_actions (
    id              TEXT PRIMARY KEY,
    action_type     TEXT NOT NULL,              -- 'reminder' | 'follow_up' | 'recap' | 'agent_task' | 'event_notify'
    title           TEXT NOT NULL,              -- résumé humain
    trigger_at      TEXT NOT NULL,              -- prochaine exécution ISO 8601
    recurrence      TEXT,                       -- RRULE si récurrent
    condition       TEXT,                       -- JSON, optionnel
    payload         TEXT NOT NULL,              -- JSON
    target_chat     TEXT NOT NULL DEFAULT 'group',
    status          TEXT DEFAULT 'pending',     -- 'pending' | 'firing' | 'done' | 'cancelled' | 'failed'
    last_fired_at   TEXT,
    last_error      TEXT,
    fire_count      INTEGER DEFAULT 0,
    created_by      TEXT REFERENCES members(id),
    created_at      TEXT DEFAULT (datetime('now'))
);
CREATE INDEX idx_sched_fire ON scheduled_actions(trigger_at) WHERE status = 'pending';
CREATE INDEX idx_sched_status ON scheduled_actions(status);
```

### 5.4 `agent_sessions` — état multi-turn

```sql
CREATE TABLE agent_sessions (
    id              TEXT PRIMARY KEY,
    member_id       TEXT REFERENCES members(id),
    last_message_at TEXT NOT NULL,
    expires_at      TEXT NOT NULL,              -- last_message_at + 60 min
    messages        TEXT NOT NULL,              -- JSON array
    pending_action  TEXT,                       -- JSON, optionnel
    created_at      TEXT DEFAULT (datetime('now'))
);
CREATE INDEX idx_sessions_member ON agent_sessions(member_id, expires_at);
```

Une session est liée à un **membre** (pas au groupe entier) — évite que A dise "ok" alors que c'est B qui parlait à l'agent. Filtrée à la lecture par `WHERE expires_at > now()`, pas de GC actif.

### 5.5 Extensions à `settings`

```sql
INSERT INTO settings VALUES ('proactive_mode', 'false');
INSERT INTO settings VALUES ('proactive_consent_at', '');
INSERT INTO settings VALUES ('proactive_max_per_hour', '3');
INSERT INTO settings VALUES ('auto_memory', 'false');
INSERT INTO settings VALUES ('auto_memory_consent_at', '');
INSERT INTO settings VALUES ('agent_quota_used_month', '0');
INSERT INTO settings VALUES ('web_search_quota_used_month', '0');
INSERT INTO settings VALUES ('agent_persona', 'default');
INSERT INTO settings VALUES ('ical_token', '');
```

### 5.6 Vectorize : index `grumps-chat-rag`

Pas une table SQL — binding CF. Configuration `wrangler.toml` :
```toml
[[vectorize]]
binding = "CHAT_RAG"
index_name = "grumps-chat-rag"
# 1024 dim (bge-m3), métrique cosine
```

Chaque vecteur stocké :
```json
{
  "id": "msg_<workspace_slug>_<message_id>",
  "values": [/* 1024 floats */],
  "metadata": {
    "workspace_slug": "x7k9m2p4",
    "platform": "telegram",
    "sender_member_id": "...",
    "sender_name": "Alice",
    "text": "...",
    "timestamp": "2026-04-19T14:23:00Z"
  }
}
```

Index unique partagé entre tous les workspaces, recherches **strictement filtrées** par `workspace_slug` (isolation garantie).

### 5.7 Migrations

3 nouveaux fichiers SQL :
- `migrations/0002_memory.sql` — `memory_entries` + FTS + triggers
- `migrations/0003_calendar.sql` — `events`
- `migrations/0004_scheduling.sql` — `scheduled_actions` + `agent_sessions` + 9 nouvelles `settings`

Tous idempotents (`CREATE TABLE IF NOT EXISTS`, `CREATE INDEX IF NOT EXISTS`).

Appliquées :
- À la création de chaque nouveau workspace par `provisioning.rs`
- Aux workspaces existants via `scripts/migrate_workspaces.sh` (one-shot)

---

## 6. Couche mémoire

### 6.1 Mémoire structurée (mode 1, défaut)

#### Sources d'écriture
| Trigger | Source | Confiance |
|---|---|---|
| `@grumps souviens-toi que ...` (commande explicite) | chat-explicit | 1.0 |
| `@grumps note Pierre est en congé jusqu'au 25` (LLM extrait via tool `save_memory`) | chat-explicit | 1.0 |
| Création/édition via SPA (page Mémoire) | web | 1.0 |
| Auto-extraction (mode 2) | chat-auto | 0.5–0.95 |
| Sauvegarde par l'agent durant une boucle (`save_memory` tool) | agent | 0.9 |

#### Cycle de vie
- **Pinned** : entry incluse systématiquement dans le contexte agent (faits stables)
- **expires_at** : entry filtrée à la lecture une fois la date passée (`WHERE expires_at IS NULL OR expires_at > now()`)
- **Édition** : page web + commandes chat (`@grumps oublie ...`, `@grumps modifie ...`)

#### Lecture (tool agent `query_memory`)
1. Si query courte/ciblée → FTS5 sur `key` + `value`
2. Sinon → fetch entries `pinned=1` + entries `kind` filtré → renvoyé brut à Sonnet
3. Pour workspaces très gros (>500 entries) : embeddings `value` (V1.1, pas au lancement)

### 6.2 Auto-extraction (mode 2, opt-in)

#### Activation
- Settings page web : toggle "Mémoire automatique" → modal de consentement RGPD
- Modal explique : *"Grumps va analyser chaque message du groupe pour en extraire des faits, décisions, préférences. Aucun message n'est stocké brut, seuls les éléments mémorisables sont conservés. Tous les membres seront informés. Tu peux désactiver à tout moment."*
- Sur activation : `auto_memory_consent_at = now()`, message annonce dans le groupe : *"📌 Mémoire automatique activée par Alice. Je vais désormais retenir les faits importants partagés ici."*
- Désactivation : message annonce aussi (transparence)

#### Pipeline
```
Message reçu (n'importe lequel)
    │
    ├─ Skip si : commande structurée déjà fast-pathée
    ├─ Skip si : @mention bot (sera traité par agent loop)
    ├─ Skip si : msg < 4 mots
    │
    ▼
Gemini Flash classifier (~$0.0002/msg)
    │ Prompt : "Ce message contient-il un fait, décision, préférence, info personne,
    │          qui mérite d'être mémorisé ? Réponds JSON {is_memorable, kind, key, value,
    │          related_member, confidence}"
    │
    ├─ is_memorable=false ou confidence < 0.5 → ignore
    ▼
INSERT memory_entries (source='chat-auto', confidence=Gemini)
    │
    ▼
Pas d'annonce dans le chat. Visible dans la page Mémoire web,
distingué visuellement (badge "AUTO"), avec actions "garder / éditer / supprimer".
```

#### Garde-fous
- **Soft cap** : max 1000 messages classés/jour/workspace (~$0.20/jour) → arrête jusqu'au lendemain si atteint
- **Stop-words** : skip messages < 4 mots, médias seuls, plus d'1 an
- **Privacy** : badge persistant en haut du SPA "Mémoire auto active" + indication chaque message bot
- **Right to forget** : `@grumps oublie tout sur moi` → `DELETE WHERE related_member = me OR created_by = me`
- **Export RGPD** : route `GET /api/w/:slug/memory/export` (JSON complet)

### 6.3 RAG sur historique chat (mode 3, défaut)

#### Ingestion (toujours active dès workspace créé)
```
Message reçu → si text length > 20 chars et pas commande structurée :
  call Workers AI binding : env.AI.run('@cf/baai/bge-m3', { text })
  → vector 1024-dim
  upsert dans Vectorize index 'grumps-chat-rag' avec metadata workspace_slug
```

Coût : Workers AI 10K neurons/jour gratuit puis $0.011/1M tokens. ~$0.05/jour pour 100 groupes.

#### Tool agent `query_chat_history(query, time_range?)`
```
1. Embed la query → vector
2. Vectorize.query(vector, topK=10, filter={workspace_slug, optional time_range})
3. Renvoie liste de [{sender, timestamp, text, score}] à Sonnet
4. Sonnet décide quoi en faire (citer, résumer, ignorer)
```

#### Cas d'usage
- *"@grumps qu'est-ce qu'on a dit sur le voyage en Italie ?"* → query → résumé
- *"@grumps quand est-ce que Pierre a dit qu'il rentrait ?"* → query → réponse précise

#### Rétention par plan
- Free : 30 jours (ancienneté max des vecteurs)
- Pro : 1 an
- Business : illimité

Nettoyage : action planifiée récurrente système, créée à la migration de chaque workspace.

---

## 7. Moteur de scheduling

### 7.1 Architecture sans polling

Aucun cron continuel. Tout repose sur les **Durable Objects avec alarmes natives CF**.

```
Création d'une scheduled_action :
   User → SPA/chat → Worker
                       │
                       ▼
              INSERT D1 scheduled_actions
                       │
                       ▼
   Worker.RPC → WorkspaceScheduler DO (id_from_name = workspace_slug)
                       │
                       ▼
              DO.schedule(action_id, trigger_at)
                       │
                       ▼
       DO compare avec son alarme courante :
         si nouvelle est plus tôt → setAlarm(new)
         sinon → no-op
                       │
                       ▼
            DO se rendort, zéro coût, attente

⏰ À trigger_at exactement :
   DO.alarm() handler se réveille
        │
        ▼
   SELECT FROM D1 scheduled_actions
     WHERE workspace_slug=this AND trigger_at <= now()
     AND status='pending' LIMIT 50
        ▼
   for each due : evaluate condition → execute → mark done/recur
        ▼
   Compute next alarm = MIN(trigger_at WHERE pending)
        ▼
   if exists : setAlarm(next) ; else : no alarm
```

### 7.2 Implémentation DO

```rust
// crates/scheduler/src/workspace_scheduler.rs
#[durable_object]
pub struct WorkspaceScheduler {
    state: State,
    env: Env,
}

#[durable_object]
impl DurableObject for WorkspaceScheduler {
    fn new(state: State, env: Env) -> Self { Self { state, env } }

    async fn fetch(&mut self, req: Request) -> Result<Response> {
        let body: ScheduleRpc = req.json().await?;
        match body {
            ScheduleRpc::Schedule { trigger_at, .. } => {
                let current = self.state.storage().get_alarm().await?;
                if current.is_none() || trigger_at < current.unwrap() {
                    self.state.storage().set_alarm(trigger_at).await?;
                }
            }
            ScheduleRpc::Reschedule => {
                let next = scheduler::query_next_pending(&self.env, &self.workspace_slug).await?;
                match next {
                    Some(ts) => self.state.storage().set_alarm(ts).await?,
                    None => self.state.storage().delete_alarm().await?,
                }
            }
        }
        Response::ok("scheduled")
    }

    async fn alarm(&mut self) -> Result<Response> {
        let due = scheduler::query_due_actions(&self.env, &self.workspace_slug).await?;
        for action in due {
            scheduler::execute(&self.env, &action).await?;
            scheduler::mark_done_or_recur(&self.env, &action).await?;
        }
        let next = scheduler::query_next_pending(&self.env, &self.workspace_slug).await?;
        if let Some(ts) = next {
            self.state.storage().set_alarm(ts).await?;
        }
        Response::ok("fired")
    }
}
```

### 7.3 Cohérence D1 ↔ DO

Write path robuste, retry + rollback :

```rust
let action_id = d1::insert_scheduled_action(...).await?;
let do_stub = env.durable_object("WS_SCHEDULER")?.id_from_name(&slug)?.get_stub()?;
let rpc_result = retry_3x(|| do_stub.fetch(...)).await;
if rpc_result.is_err() {
    d1::delete_scheduled_action(action_id).await?;
    return Err(GrumpsError::SchedulingFailed);
}
```

→ Aucune divergence silencieuse possible. **Pas de cron de safety net, pas de cron de GC.**

### 7.4 Types d'actions et payloads

| `action_type` | `payload` (JSON) | Quand utilisé |
|---|---|---|
| `reminder` | `{"text":"Appeler maman", "creator_member_id":"..."}` | Rappel passif simple |
| `event_notify` | `{"event_id":"evt_...", "lead_minutes":30}` | Notif X min avant un event du calendrier |
| `recap` | `{"scope":"weekly"}` | Recap auto |
| `follow_up` | `{"target_type":"todo|message", "target_id":"...", "prompt":"..."}` | Suivi conditionnel (B) |
| `agent_task` | `{"instruction":"...", "creator_member_id":"..."}` | Action agentique différée (C) |

### 7.5 Évaluation de condition (suivis B)

Format JSON, **5 types prédéfinis au lancement**. Pas de DSL custom. Évaluateur dans `crates/scheduler/condition.rs`.

| `type` | Champs payload | Sémantique |
|---|---|---|
| `no_message_matching` | `since`, `match_keywords[]`, `min_message_count` | Fire si moins de N messages contenant les keywords depuis la date |
| `member_active_after` | `member_id`, `after` | Fire dès que ce member a posté un message après cette date |
| `todo_status` | `todo_id`, `status_not` (ou `status_is`) | Fire si la todo a/n'a pas ce statut |
| `member_inactive_for` | `member_id`, `duration_seconds` | Fire si le member n'a pas posté depuis cette durée |
| `keyword_appeared` | `keywords[]`, `since` | Fire dès qu'un message du chat contient un de ces keywords après la date |

```jsonc
// Exemple : "Si personne n'a répondu pour le restau d'ici jeudi"
{
  "type": "no_message_matching",
  "since": "2026-04-19T14:00:00Z",
  "match_keywords": ["restau", "restaurant"],
  "min_message_count": 1
}

// Exemple : "Quand Marc revient de vacances le 25"
{
  "type": "member_active_after",
  "member_id": "m_marc",
  "after": "2026-04-25T00:00:00Z"
}

// Exemple : "Si la todo #42 n'est toujours pas done dans 3 jours"
{
  "type": "todo_status",
  "todo_id": "t_42",
  "status_not": "done"
}
```

Si `condition` évalue `false` :
- Action reportée de `payload.recheck_in` (défaut 1h)
- Jusqu'à `payload.give_up_after` (défaut 7j)
- Puis `status='cancelled'`

Type inconnu → log warning + considère condition `true` (fail open).

### 7.6 Recurrence (RRULE)

Format RFC 5545 RRULE. Crate `rrule` (à valider wasm32 compatibility ; sinon implémentation manuelle des cas usuels).

Cas supportés au lancement :
- `FREQ=DAILY` — tous les jours
- `FREQ=WEEKLY;BYDAY=MO` — tous les lundis
- `FREQ=WEEKLY;BYDAY=FR;INTERVAL=2` — un vendredi sur deux
- `FREQ=MONTHLY;BYMONTHDAY=1` — tous les 1ers du mois
- `FREQ=YEARLY;BYMONTH=3;BYMONTHDAY=15` — tous les 15 mars

À chaque `done`, si `recurrence` non null :
```
next = compute_next_occurrence(recurrence, now)
UPDATE trigger_at = next, status = 'pending', fire_count += 1
```

### 7.7 Idempotence et reprise sur erreur

- `status='firing'` est un verrou DO-handled : si une exécution crash partiellement, prochain alarm récupère
- `fire_count > 5` → `status='failed'`, écrit `last_error`, ne réessaie pas

### 7.8 Création depuis le chat

Trois chemins :

1. **Commande structurée** (regex fast-path) :
   ```
   REMIND tomorrow 9am: Call dentist
   REMIND every monday 9am: Standup
   ```

2. **Via @grumps** (NLU → Sonnet → tool `schedule_action`)

3. **Via SPA** (page Scheduled Actions) — formulaire CRUD complet, picker date/heure, RRULE builder visuel

---

## 8. Boucle agent + tool use

### 8.1 Routing des messages

```
Message reçu
    │
    ├─ Match regex commande structurée ? → fast-path déterministe, zéro LLM
    │
    ├─ Reply task card ? → fast-path déterministe, zéro LLM
    │
    ├─ Session multi-turn active (member_id) ?
    │    OUI → bypass classifier, direct AGENT LOOP (Sonnet)
    │
    └─ Sinon → Gemini Flash classifier (~$0.0002, ~200ms)
              │
              ├─ intent simple + confidence > 0.85 → CRUD direct (zéro Sonnet)
              └─ complex_agent_task OR confidence ≤ 0.85 → AGENT LOOP (Sonnet)
```

### 8.2 Le classifier Gemini Flash

```
Prompt : "You classify a chat message addressed to an assistant in a group.
Return JSON :
{
  intent: 'create_todo' | 'create_note' | 'create_event' | 'create_reminder'
        | 'list_todos' | 'list_notes' | 'list_events' | 'list_reminders'
        | 'mark_done' | 'delete_item' | 'workspace_link'
        | 'complex_agent_task',
  confidence: number 0..1,
  args: { /* champs extraits selon l'intent, ou {} */ }
}

Use 'complex_agent_task' if any of :
- needs to search the web
- needs to recall past conversations or workspace memory
- needs multi-step reasoning or planning
- is ambiguous (could mean several things)
- is a question that needs reasoning beyond a list lookup
- creates something with conditions or follow-ups
- is creative/open-ended"
```

Validation des args avant exec (si CRUD direct) :
- Schema JSON validé (`jsonschema` crate)
- Champs requis présents, types corrects
- Member references existent dans la table `members`
- Si validation échoue → escalation Sonnet (jamais d'erreur user)

### 8.3 Anatomie d'un tour d'agent (Sonnet)

```
1. Récupère ou crée session (member_id, ws)
   - lookup table agent_sessions WHERE member_id AND expires_at>now()
   - si miss → nouvelle session, expires=now+1h

2. Construit le contexte :
   ┌─────────────────────────────────────────────┐
   │ system_prompt (ws-aware, persona-aware)     │
   │ + workspace_info (name, members list)       │
   │ + memory pinned (5-50 entries, max ~5K tok) │
   │ + session.messages (history multi-turn)     │
   │ + new_user_message                          │
   └─────────────────────────────────────────────┘

3. POST api.anthropic.com /v1/messages
   model: claude-sonnet-4-6
   max_tokens: 1024
   tools: [TOOL_SCHEMAS]
   tool_choice: auto

4. Réponse :
   ├─ stop_reason="end_turn" → final, envoyer text au groupe
   └─ stop_reason="tool_use" → exécuter chaque tool_call
       - parse args, run, capture result
       - append messages : [assistant tool_use, user tool_result]
       - GOTO step 3

5. Garde-fous :
   - Max 5 itérations tool_use
   - Timeout total 30s par invocation worker (CF limit)
   - Tokens cumulés > 50K → arrêt forcé

6. Persist :
   - UPDATE agent_sessions SET messages=full_history, last_message_at=now()
   - INSERT activity_log
```

### 8.4 Catalogue des tools

```jsonc
[
  { "name": "query_memory", /* search structured memory by query, kind?, limit */ },
  { "name": "query_chat_history", /* semantic search past messages, query, from?, to?, limit */ },
  { "name": "save_memory", /* persist fact/decision/person info, kind, value, related_member?, expires_at?, pinned? */ },
  { "name": "create_todo", /* title, assignee?, deadline?, priority?, tags? */ },
  { "name": "create_note", /* title, content (markdown) */ },
  { "name": "create_event", /* title, starts_at, ends_at?, all_day?, location?, attendees?, recurrence? */ },
  { "name": "create_reminder", /* text, trigger_at, recurrence? */ },
  { "name": "schedule_action", /* action_type, trigger_at, recurrence?, condition?, payload */ },
  { "name": "list_calendar", /* from, to, types? */ },
  { "name": "web_search", /* query, count?, freshness? */ },
  { "name": "send_message", /* extra messages mid-flow */ }
]
```

### 8.5 System prompt (template)

```
You are Grumps, an AI assistant living inside the messaging group "{ws.name}"
({platform}). The group has {N} members. You serve the group, not individuals —
all your messages are visible to everyone.

PERSONA: {persona}
  default → "Helpful, concise, dry humor when fitting. No emojis except in lists/cards.
             Tagline you embody: 'Gets it done. No small talk.'"
  playful → "Warm, slightly playful, light emojis welcome."
  formal  → "Precise, polite, no emojis."

LANGUAGE: Respond in {ws.lang} unless the user writes in another language.

MEMORY POLICY:
- You have access to the workspace's persistent memory via tools.
- Pinned memories below are always relevant. Use them.
- For questions about past events, use query_chat_history.
- Save things to memory only if they have lasting value.

PINNED MEMORY:
{list of memory_entries WHERE pinned=1, formatted as bullets}

MEMBERS:
{list of active members with roles}

CURRENT DATETIME: {now in ws.timezone}

WORKSPACE SETTINGS:
- Proactive mode: {on|off}
- Auto memory: {on|off}
- Quotas remaining this month: agent_calls={X}/{Y}, web_search={A}/{B}

RULES:
- When using create_* tools, do NOT also restate the result in your final message — just confirm briefly.
- For web_search results, always cite source URLs.
- If the user is ambiguous, ask before acting (one short question).
- Never schedule, save, or modify on someone else's behalf without explicit consent in the message.
```

### 8.6 Conversation multi-turn

Cas typique :
```
Alice 14:23 : @grumps cherche un resto italien sympa pour ce soir
Bot   14:23 : J'en ai trouvé 3 :
              1. Trattoria Da Mario — 4.6/5, République, 25€/pers
              2. Pizzeria Popolare — 4.4/5, Sentier, 18€
              3. Da Cesare — 4.5/5, Marais, 35€
              Je propose au groupe ?
Alice 14:24 : oui le 1
Bot   14:24 : Posté dans le groupe. Un event ce soir 20h ?
Alice 14:25 : ouais 20h30 stp
Bot   14:25 : 📅 Event créé : Trattoria Da Mario, ce soir 20h30.
```

Côté technique :
- Session `(member_id=alice, ws=...)` créée au 1er msg, expires à 14:23 + 1h
- À chaque message d'Alice dans la fenêtre, on retrouve la session, on injecte tout l'historique → Sonnet a le contexte complet
- Sessions invisibles aux autres membres ; B ne peut pas répondre à la place d'A par accident
- Garbage-collection à la lecture (`WHERE expires_at > now()`)

### 8.7 Garde-fous coûts

| Garde-fou | Valeur |
|---|---|
| Max tool turns / invocation | 5 |
| Max tokens cumulés / invocation | 50 000 |
| Quota Sonnet par workspace | 200/mois Free, 1000 Pro, 5000 Business |
| Quota web_search par workspace | 5/mois Free, 50 Pro, 500 Business |
| Cache prompt Sonnet | Activé sur system_prompt + memory pinned (TTL 5 min) |
| Timeout dur invocation worker | 30s |

Coût moyen pour un tour Sonnet (avec cache) : **~$0.005/conversation**.

---

## 9. Calendrier

### 9.1 Aggregation

Le calendrier n'est pas une 5ᵉ table — c'est une **vue agrégée** qui union 4 sources :

```rust
pub struct CalendarItem {
    pub id: String,                   // "todo:42" | "evt:xyz" | "rmd:..." | "sch:..."
    pub source: CalendarSource,       // Todo | Event | Reminder | ScheduledAction
    pub title: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub all_day: bool,
    pub location: Option<String>,
    pub color: String,                // teal | brick | cream | slate
    pub member_id: Option<String>,
    pub recurrence: Option<String>,
    pub editable: bool,
    pub url: String,
}

pub async fn list_calendar(
    env: &Env, ws: &Workspace,
    from: DateTime<Utc>, to: DateTime<Utc>,
    members: Option<Vec<String>>,
    sources: Option<Vec<CalendarSource>>,
) -> Result<Vec<CalendarItem>>
```

4 queries D1 parallèles, union, expansion des récurrents (RRULE entre `from..to`), tri par `starts_at`.

### 9.2 Mapping sources → couleurs

| Source | Couleur défaut | Override possible |
|---|---|---|
| Event | `teal` | oui (champ `color` table) |
| Todo avec deadline | `brick` | non |
| Reminder passif | `cream` (border slate) | non |
| ScheduledAction | `slate-300` | non |

### 9.3 Endpoints

```
GET /api/w/:slug/calendar?from=...&to=...&members=...&sources=...
  → 200 [CalendarItem, ...]

POST /api/w/:slug/calendar/ical-token       (admin)
  → 201 { url: "https://api.grumps.io/cal/x7k9m2p4.ics?t=...", revoke_url: "..." }

DELETE /api/w/:slug/calendar/ical-token     (admin)
  → 204

GET /cal/:slug.ics?t=<jwt>                  (public, auth via token)
  → 200 text/calendar (VCALENDAR avec VEVENT/VTODO, RRULE inclus)
```

### 9.4 iCal export

Format standard RFC 5545. JWT token stocké dans `settings.ical_token` (un seul valide à la fois). Pas d'expiration côté JWT mais révocable.

**Mécanisme de révocation** : le JWT seul ne suffit pas pour authentifier. À chaque requête `GET /cal/:slug.ics?t=<jwt>`, l'endpoint :
1. Vérifie la signature JWT (rejette si signature invalide)
2. Lit `settings.ical_token` du workspace
3. Compare le token reçu à celui stocké
4. Si différent (token révoqué/régénéré) → 401

→ Régénérer ou révoquer = wiper/changer `settings.ical_token`. Les anciennes URLs sont mortes immédiatement.

```ics
BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//Grumps//Calendar 1.0//EN
NAME:Grumps — {ws.name}
X-WR-CALNAME:Grumps — {ws.name}
X-PUBLISHED-TTL:PT15M

BEGIN:VEVENT
UID:evt_xyz@grumps.io
DTSTAMP:20260419T140000Z
DTSTART:20260424T180000Z
DTEND:20260424T200000Z
SUMMARY:Trattoria Da Mario
LOCATION:République, Paris
DESCRIPTION:Reserved by Alice via @grumps
URL:https://grumps.io/w/x7k9m2p4/events/evt_xyz
RRULE:FREQ=WEEKLY;BYDAY=FR
END:VEVENT

BEGIN:VTODO
UID:todo_42@grumps.io
SUMMARY:Acheter cadeaux Noël
DUE:20261220T000000Z
PRIORITY:1
END:VTODO

END:VCALENDAR
```

Notes :
- Pas de PUSH : Google poll ~12h, Apple ~configurable, Outlook ~3h. Documenté dans Help.
- Range : 1 an passé + 1 an futur (clients filtrent côté client). Tronqué à 6 mois passé si > 5MB.
- Récurrents : RRULE émis tel quel, le client iCal expand.

### 9.5 UI SPA

Page `/w/:slug/calendar` avec 3 vues :
- **Mois** (par défaut desktop) : grille 7×6
- **Semaine** (desktop) : grille horaire 7 colonnes × 24h
- **Agenda / Liste** (mobile-first) : liste verticale chronologique

Interactions :
- Clic sur item → side panel détails + actions
- Clic sur jour vide → modal "créer event/todo/reminder ce jour"
- Drag & drop → modifie `starts_at`/`deadline` (items `editable=true` uniquement)
- Filter par membre (checkboxes header, persisté URL)
- Filter par source (dropdown multi-select)

Composant maison Leptos (pas de FullCalendar JS), ~600 lignes pour 3 vues + interactions. Drag = ~150 lignes JS-via-WASM.

### 9.6 Direction artistique

**Style warm brutalism** (cohérent design system existant) :
- Pas de Material Light ni shadcn standard
- Slab serif (Recoleta/Roboto Slab) pour titres et numéros
- Mono (JetBrains/IBM Plex Mono) pour heures/dates/codes
- Inter pour body
- Palette : cream (#F5EBDD), brick (#C44536), teal (#0D7377), slate-900 (#1A1A1A)
- Bordures épaisses 1.5–2px noires
- Ombres dures `box-shadow: 3px 3px 0 #1A1A1A`, jamais de blur
- Hover → ombre se réduit à `1px 1px 0` (effet pressé)
- Drag → rotation `-2deg`, ombre étendue (Post-it)
- Numéros de date 3rem en haut-gauche
- Aujourd'hui : surlignage teal translucide + badge slab uppercase
- Pictos SVG monochromes faits main, pas d'emojis flat

Anti-AI-slop checklist :
- ❌ Pas de gradients pastel
- ❌ Pas de shadows blurry
- ❌ Pas d'emojis dans titres d'item
- ❌ Pas de rounded-2xl partout
- ❌ Pas de barres de progression circulaires animées
- ❌ Pas de skeletons gris animés
- ❌ Pas de mode sombre auto-inversé

Tests visuels : screenshots Playwright vs baseline approuvée.

---

## 10. Web search tool

### 10.1 Provider abstraction

```rust
#[async_trait]
pub trait WebSearchProvider: Send + Sync {
    async fn search(&self, query: &str, count: u8, freshness: &str) -> Result<Vec<Hit>>;
}

pub struct BraveProvider { api_key: String }
pub struct SerperProvider { api_key: String }
pub struct SearXNGProvider { instance_url: String, api_key: Option<String> }

// Boot-time choice via env var :
let provider: Box<dyn WebSearchProvider> = match env.var("WEB_SEARCH_PROVIDER")?.to_string().as_str() {
    "searxng" => Box::new(SearXNGProvider {
        instance_url: env.var("SEARXNG_URL")?.to_string(),
        api_key: env.secret("SEARXNG_API_KEY").ok(),
    }),
    "serper" => Box::new(SerperProvider::new(env.secret("SERPER_API_KEY")?)),
    _ => Box::new(BraveProvider::new(env.secret("BRAVE_API_KEY")?)),
};
```

### 10.2 Provider par défaut : Brave Base

- Tier "Data for Search → Base" : **$3/CPM** ($0.003/req)
- Tier gratuit Brave : 2000 req/mois → couvre tout le tier Free Grumps
- Au-delà : ~$50/mois pour 1000 workspaces

### 10.3 Provider OSS alternatif : SearXNG

- Open source AGPLv3
- Hébergement séparé (Hetzner CX11 ~4,51 €/mois, Fly.io free tier, ou self-host)
- Migration : changer `WEB_SEARCH_PROVIDER=searxng` + `SEARXNG_URL=...`
- Documentation hosting dans `docs/self-hosting.md` (à écrire)

### 10.4 Implémentation

```rust
pub async fn web_search(env: &Env, ws: &Workspace, args: WebSearchArgs)
    -> Result<WebSearchResult> {
    // 1. Check quota workspace
    let plan = billing::get_plan(env, ws).await?;
    let used = settings::get_int(env, ws, "web_search_quota_used_month").await?;
    let limit = match plan { Plan::Free => 5, Plan::Pro => 50, Plan::Business => 500 };
    if used >= limit { return Err(GrumpsError::QuotaExceeded { ... }); }

    // 2. Cache KV (TTL 1h, clé = sha256(query+freshness))
    let cache_key = format!("ws:{}", sha256(&args.query, &args.freshness));
    if let Some(cached) = env.kv("KV")?.get(&cache_key).json().await? {
        return Ok(cached);
    }

    // 3. Call provider
    let provider = web_search_provider(env)?;
    let hits = provider.search(&args.query, args.count.unwrap_or(5).min(10), args.freshness.as_deref().unwrap_or("all")).await?;

    // 4. Cache + bump quota
    let result = WebSearchResult { query: args.query, results: hits };
    env.kv("KV")?.put(&cache_key, &result)?.expiration_ttl(3600).execute().await?;
    settings::increment(env, ws, "web_search_quota_used_month", 1).await?;
    activity_log::record(env, ws, "agent.web_search", &args.query).await?;

    Ok(result)
}
```

### 10.5 Quotas reset

1er du mois 00:00 UTC, via une `scheduled_action` système récurrente (`FREQ=MONTHLY;BYMONTHDAY=1;BYHOUR=0;BYMINUTE=0`) créée à la migration de chaque workspace. L'executor reset `agent_quota_used_month` et `web_search_quota_used_month` à 0.

---

## 11. Mode proactif (c) — opt-in

### 11.1 Activation

Settings page (admin only) : toggle "Mode proactif", modal de confirmation, ON :
- `settings.proactive_mode = true`
- `settings.proactive_consent_at = now()`
- Message annonce groupe : *"📣 Mode proactif activé par Alice. Je peux désormais intervenir si la conversation touche un sujet utile, sans qu'on m'appelle. /quiet pour me faire taire, ou désactivez dans les settings."*

**Réservé Pro+** (toggle grisé sur Free, lien upgrade).

### 11.2 Pipeline d'intervention

```
Message reçu (mode_proactive=true)
    │
    ├─ Skip si : commande structurée (déjà fast-pathée)
    ├─ Skip si : @mention bot (passe par agent loop normal)
    ├─ Skip si : reply à un msg du bot
    ├─ Skip si : intervention dans la dernière 1 minute (cooldown KV TTL 60s)
    ├─ Skip si : 3 interventions cette heure (rate limit KV TTL 2h)
    ├─ Skip si : msg < 4 mots ou texte vide
    ├─ Skip si : sender = bot (auto-référence)
    │
    ▼
Gemini Flash classifier — appel "should_intervene"
    Inputs : message, last 5 messages context, summary of pinned memory (tronqué 1K tok)
    Prompt : "Should an assistant who knows {pinned} chime in here?
              Respond JSON {should_intervene: bool, reason: string, urgency: low|medium|high}"
    Coût : ~$0.0003/msg
    │
    ├─ should_intervene=false → ignore, fin
    │
    ▼
Sonnet "filter and respond" call
    Inputs : full system prompt + memory + last 10 messages + the trigger message
    + classifier's reason (en hint)
    + tools complète
    + INSTRUCTION : "You may stay silent. Only respond if you have a SPECIFIC,
      VERIFIABLE, CONCISE thing to add. If unsure, return empty."
    │
    ├─ Réponse non-vide → POST au groupe, log activity, increment counter
    └─ Réponse vide ou phrase d'incertitude → ignore
```

### 11.3 Garde-fous

| Risque | Mitigation |
|---|---|
| Bot devient pénible | Rate limit 3/h + double filtre + cooldown 60s |
| Bot s'auto-référence | Skip si `sender = bot` |
| Bot intervient dans une discussion sensible (conflit en cours) | Sonnet a règle système : "skip if last 5 messages indicate active interpersonal conflict, anger, or grief" |
| Coût explose | Hard cap quota mensuel partagé réactif+proactif. Au-delà, désactive proactif d'office et notifie admin. |

### 11.4 Commandes utilisateur

| Commande chat | Effet |
|---|---|
| `@grumps tais-toi` ou `@grumps quiet` | Désactive proactif pour 24h (KV TTL 86400) |
| `@grumps reviens` ou `@grumps unquiet` | Réactive avant 24h |
| `@grumps désactive le mode proactif` | NLU detect → flip setting (admin only) |

### 11.5 Coût estimé

Pour un groupe actif (300 msg/jour) en mode proactif :
- Classifier Flash sur ~60% des messages (180 calls × $0.0003) = $0.054/jour
- Sonnet filter sur ~10% des classifier passing (18 calls × $0.005) = $0.09/jour
- → **~$4-5/mois par workspace en mode proactif**

À 1000 workspaces × 10% activent = ~$400-500/mois supplémentaires.

---

## 12. UI workspace — nouvelles pages

### 12.1 Routes ajoutées

```
/w/:slug/calendar           ★ NOUVEAU
/w/:slug/memory             ★ NOUVEAU
/w/:slug/scheduled          ★ NOUVEAU
/w/:slug/events/:id         ★ NOUVEAU
/w/:slug/memory/:id         ★ NOUVEAU
/w/:slug/scheduled/:id      ★ NOUVEAU
```

### 12.2 Sidebar étendue

```
🏠 Vue d'ensemble       /w/:slug
✓  Todos                /w/:slug/todos
📝 Notes                /w/:slug/notes
📁 Fichiers             /w/:slug/files
📅 Calendrier           /w/:slug/calendar          ★
🧠 Mémoire              /w/:slug/memory            ★
⏰ Actions planifiées   /w/:slug/scheduled         ★
🕓 Historique           /w/:slug/history
⚙️  Paramètres          /w/:slug/settings
```

### 12.3 Page Mémoire `/w/:slug/memory`

- Filtres chip-row : Tout / Personnes / Faits / Décisions / Préférences / Lieux
- Filtre source : Tout / Manuel / Auto
- Carte mémoire : badge `kind`, badge "AUTO" si `source=chat-auto`, 📌 si `pinned`
- Confiance et expiration affichés sur les auto-extraits
- Actions inline : éditer (modal markdown), épingler, supprimer
- Bouton "+ Nouvelle entrée" → modal form
- Bouton "Exporter JSON" (RGPD)
- Bouton "Tout supprimer" (admin, double confirmation)

### 12.4 Page Actions planifiées `/w/:slug/scheduled`

- Filtres : Toutes / Rappels / Suivis / Recaps / Tâches agent
- État : Actives / Pause / Historique
- Carte action : titre humain, icône type, prochain trigger, compteur exécutions
- Actions inline : pause, éditer, supprimer, exécuter maintenant
- Bouton "+ Planifier une action" → modal form complet (type, prompt, date/heure, RRULE builder visuel, condition)

### 12.5 Page Calendrier `/w/:slug/calendar`

Détaillée § 9.5 et § 9.6.

### 12.6 Page Settings `/w/:slug/settings` — extensions

Nouvelles sections :
- **Agent IA** : personnalité (default/playful/formal), mode proactif (Pro+ only), mémoire automatique, lien "voir consentement"
- **Calendrier** : URL d'abonnement iCal, boutons Copier/Régénérer/Révoquer
- **Quotas ce mois** : appels agent X/Y, recherches web X/Y, stockage R2 X/Y

### 12.7 Vue d'ensemble `/w/:slug` — extension

2 widgets ajoutés en bas :
- **Cette semaine au calendrier** : mini-grille 7 jours
- **Ce que je sais sur le groupe** : top 5 mémoires épinglées

→ Donne le sentiment d'un agent qui "connaît" le groupe dès le dashboard.

### 12.8 Crate spa — nouveaux fichiers

```
crates/spa/src/
├── pages/
│   ├── calendar.rs              ★
│   ├── memory.rs                ★
│   ├── memory_detail.rs         ★
│   ├── scheduled.rs             ★
│   └── scheduled_detail.rs      ★
├── components/
│   ├── calendar/
│   │   ├── month.rs             ★
│   │   ├── week.rs              ★
│   │   ├── agenda.rs            ★
│   │   ├── item.rs              ★
│   │   └── drag.rs              ★
│   ├── memory_card.rs           ★
│   ├── scheduled_card.rs        ★
│   └── recurrence_picker.rs     ★ (RRULE builder visuel)
└── api/
    ├── calendar.rs              ★
    ├── memory.rs                ★
    └── scheduled.rs             ★
```

---

## 13. Stratégie LLM & coûts

### 13.1 Modèles utilisés

| Modèle | Rôle | Quand |
|---|---|---|
| Gemini 2.5 Flash | Classifier rapide | Mode (c) proactif, auto-extract, et routing initial — sur chaque message qualifié. ~$0.0002/msg |
| Claude Haiku 4.5 | NLU simple existant | Conservé pour fallback si Sonnet down ou rate limited |
| **Claude Sonnet 4.6** *(nouveau)* | Cerveau de l'agent | Tool use, raisonnement multi-étapes, queries mémoire complexes. ~$0.005/call |

### 13.2 Cache Anthropic

System prompt + memory pinned cachés (5 min TTL). Réduit le coût quand un workspace fait plusieurs appels rapprochés.

### 13.3 Estimation coûts mensuels (1000 workspaces actifs — ordre de grandeur)

Hypothèses : 60% Free + 35% Pro + 5% Business. Activité moyenne 50 msg/jour/workspace.

| Poste | Volume estimé | Coût mensuel |
|---|---|---|
| Sonnet (agent réactif) | ~30 calls/ws/mois × 1000 = 30K calls | ~$150 |
| Gemini Flash (routing classifier) | ~100 calls/ws/mois × 1000 = 100K calls | ~$20 |
| Gemini Flash (auto-extract, ~10% activent) | 100 ws × ~30 msg/jour éligibles × 30 = ~90K | ~$20 |
| Gemini Flash (proactif classifier, ~10% activent) | 100 ws × ~200 msg/jour éligibles × 30 = 600K | ~$120 |
| Sonnet (proactif filter, rate-limité 3/h) | ~30 calls/ws/mois × 100 ws = 3K | ~$15 |
| Workers AI embeddings (RAG ingestion) | ~150K/mois | ~$2 |
| Brave web search (Brave Base $3/CPM, après 2K free) | ~18 500 req/mois | ~$50 |
| **Total LLM + search** | | **~$370-420/mois** |

À comparer aux revenus estimés : 350 Pro × 5€ + 50 Business × 15€ = **2 500€/mois** ⇒ marge brute ~80% sur ces postes. Confortable.

> Note : ces chiffres sont des ordres de grandeur, pas une projection précise. Volumes réels à monitorer dès les premiers workspaces actifs pour ajuster les quotas.

---

## 14. Settings & opt-ins

### 14.1 Toggles workspace

| Setting | Défaut | Plan requis |
|---|---|---|
| `proactive_mode` | `false` | Pro+ |
| `auto_memory` | `false` | Tous |
| `agent_persona` | `default` | Tous (3 valeurs : default/playful/formal) |

### 14.2 Flow consentement RGPD

Pour `auto_memory` ET `proactive_mode`, modal de consentement explicite avant activation :

```
┌─────────────────────────────────────────────────────────┐
│ Activer la mémoire automatique                          │
│                                                         │
│ Si tu actives cette option :                            │
│ • Grumps va analyser chaque message du groupe           │
│ • Il va extraire les faits, décisions, préférences      │
│   qui méritent d'être mémorisés                         │
│ • Aucun message brut n'est stocké                       │
│ • Tous les membres du groupe seront notifiés            │
│ • Tu peux tout supprimer ou désactiver à tout moment    │
│                                                         │
│ Cette opération nécessite l'accord du groupe.           │
│                                                         │
│   [Annuler]                            [J'active]       │
└─────────────────────────────────────────────────────────┘
```

Sur activation : message annonce dans le groupe + `*_consent_at = now()` enregistré pour audit.

Désactivation : message annonce aussi (transparence).

### 14.3 Right to forget

Commande chat : `@grumps oublie tout sur moi` →
- Détecte le sender
- DELETE memory_entries WHERE related_member = sender OR created_by = sender
- DELETE de Vectorize les vecteurs où `metadata.sender_member_id = sender`
- Confirme avec count

Page Settings → "Zone dangereuse" → bouton "Réinitialiser la mémoire" (admin only) :
- Wipe tout `memory_entries`
- Wipe tous les vecteurs Vectorize du workspace

---

## 15. Migration & déploiement

### 15.1 Séquence

```bash
# 1. Vectorize index (one-shot)
wrangler vectorize create grumps-chat-rag --dimensions=1024 --metric=cosine

# 2. Secrets
wrangler secret put BRAVE_API_KEY
# (ANTHROPIC_API_KEY déjà set, GEMINI_API_KEY déjà set)

# 3. Update wrangler.toml :
#    - [[vectorize]] binding CHAT_RAG
#    - [ai] Workers AI binding
#    - [[durable_objects.bindings]] WS_SCHEDULER
#    - [[migrations]] new_classes ["WorkspaceScheduler"]
#    - [vars] WEB_SEARCH_PROVIDER = "brave"

# 4. Deploy worker
wrangler deploy

# 5. Migrate workspaces existants (idempotent)
./scripts/migrate_workspaces.sh

# 6. Optional : RAG backfill historique
./scripts/rag_backfill.sh

# 7. Deploy SPA
cd crates/spa && trunk build --release
wrangler pages deploy ../../dist/
```

### 15.2 Script `migrate_workspaces.sh`

```bash
#!/bin/bash
# Idempotent : applique 0002/0003/0004 à tous les workspaces existants
wrangler d1 execute grumps-index --command "SELECT slug, d1_database_id FROM workspaces_meta" --json \
  | jq -r '.[0].results[] | "\(.d1_database_id)\t\(.slug)"' \
  | while IFS=$'\t' read -r db_id slug; do
      for mig in 0002_memory 0003_calendar 0004_scheduling; do
        wrangler d1 execute "$db_id" --file="migrations/${mig}.sql" --remote || {
          echo "FAILED: $slug $mig"; exit 1
        }
      done
      # Bonus : migrer les recaps cron-based vers scheduled_actions
      wrangler d1 execute "$db_id" --file="migrations/0005_convert_recaps.sql" --remote
      echo "OK: $slug"
    done
```

### 15.3 Conversion recaps existants

`migrations/0005_convert_recaps.sql` :
```sql
-- created_by = NULL pour distinguer les actions système des actions user
-- (la colonne est nullable, REFERENCES members(id) avec NULL = pas d'auteur user)
INSERT INTO scheduled_actions (id, action_type, title, trigger_at, recurrence, payload, target_chat, created_by)
SELECT lower(hex(randomblob(16))), 'recap', 'Recap hebdomadaire',
       datetime('now', 'weekday 1', '09:00'),
       'FREQ=WEEKLY;BYDAY=MO;BYHOUR=9',
       json_object('scope', 'weekly'),
       'group', NULL
WHERE NOT EXISTS (SELECT 1 FROM scheduled_actions WHERE action_type='recap');
```

`migrations/0006_migrate_reminders.sql` (cohérence : tout passe par `scheduled_actions` + DOs) :
```sql
-- Migre les reminders actifs futurs vers scheduled_actions
INSERT INTO scheduled_actions (id, action_type, title, trigger_at, recurrence, payload, target_chat, created_by)
SELECT id, 'reminder', title, remind_at, recurrence,
       json_object('text', title, 'creator_member_id', created_by),
       'group', created_by
FROM reminders
WHERE status = 'active' AND remind_at > datetime('now')
  AND NOT EXISTS (SELECT 1 FROM scheduled_actions sa WHERE sa.id = reminders.id);

-- La table reminders devient legacy/read-only — pas droppée pour l'historique
-- Le cron-based reminder handler dans crates/worker/src/cron.rs sera désactivé
-- au prochain deploy (cf § 15.5)
```

Puis script Rust qui pour chaque workspace, après les migrations SQL :
1. SELECT `MIN(trigger_at)` parmi `scheduled_actions WHERE status='pending'`
2. RPC `WS_SCHEDULER` DO du workspace : `schedule(min_trigger_at)` pour armer la première alarme

Sans ce step, les DO ne sont jamais "réveillés" la première fois et les actions ne firent pas.

### 15.4 Backfill RAG (optionnel)

```bash
# Pour chaque workspace, embed les messages du activity_log et upsert dans Vectorize
./scripts/rag_backfill.sh
```

Coût one-shot : ~$5 pour 100 workspaces × 5000 msg historiques.

### 15.5 Rollback

Revert le commit, redéploy. Les nouvelles tables D1 restent en place mais inutilisées. Aucune donnée perdue.

---

## 16. Testing strategy

### 16.1 Unit tests (~130 nouveaux)

- `memory::entries` : CRUD + FTS — ~15 tests
- `memory::auto_extract` : pipeline avec classifier mocké — ~10 tests
- `memory::rag` : embed + query Vectorize mocké — ~8 tests
- `scheduler::condition` : tous les types + edge cases — ~20 tests
- `scheduler::recurrence` : parsing + expansion RRULE — ~25 tests (critique)
- `scheduler::executor` : dispatch par action_type — ~10 tests
- `calendar::aggregate` : union 4 sources + tri + récurrents — ~15 tests
- `calendar::ical` : génération VCALENDAR validée vs lib parser — ~10 tests
- `agent::router` : classifier mock → dispatch — ~12 tests
- `agent::loop` : boucle avec mock Anthropic — ~8 tests

### 16.2 Integration tests

`crates/worker/tests/integration.rs` avec `wrangler dev` en background. Mocks via httpmock pour Anthropic/Gemini/Brave/Vectorize/Workers AI.

Scénarios :
1. Create → schedule → fire (DO alarm)
2. Auto-extract pipeline end-to-end
3. RAG query end-to-end
4. Cascade classifier : simple → CRUD direct ; complex → Sonnet
5. Proactive intervention (mode ON, message trigger, classifier TRUE)
6. Rate limit proactif (4ème intervention/h skip)
7. iCal export (POST event → GET .ics → validation VEVENT)
8. Calendar aggregation (4 sources → vue triée)
9. Quota enforcement (51ème web_search Pro = QuotaExceeded)

CI : `cargo test --workspace --target x86_64-pc-windows-msvc`. ~5 min total.

### 16.3 E2E Playwright (manuel au début, auto après)

- Créer event calendrier → drag → reload → position persistée
- Action planifiée récurrente → vérifier visible au prochain trigger
- Activer auto-mémoire → envoyer fait → vérifier entry visible
- Export iCal → URL copiée → s'abonner Google Calendar (manuel) → vérifier

### 16.4 Tests visuels design system

Screenshots Playwright sur Calendar/Memory/Scheduled vs baseline approuvée. Pixel-diff sur changements UI.

---

## 17. Error handling

### 17.1 Hiérarchie d'erreurs

```rust
#[derive(thiserror::Error, Debug)]
pub enum GrumpsError {
    // User-facing
    #[error("Le quota {tool} est atteint ({used}/{limit}). Plan {plan}.")]
    QuotaExceeded { tool: String, used: u32, limit: u32, plan: Plan, upgrade_url: String },
    #[error("Commande non comprise : {0}")]
    InvalidCommand(String),
    #[error("Non autorisé : {0}")]
    Forbidden(String),
    #[error("La planification a échoué, réessaie.")]
    SchedulingFailed,

    // Infrastructure (logged, not shown)
    #[error("LLM provider error: {0}")]
    LlmProviderError(String),
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("External API error: {provider}: {message}")]
    ExternalApiError { provider: String, message: String },

    // Programmatique
    #[error("Internal inconsistency: {0}")]
    Internal(String),
}
```

### 17.2 Stratégies par couche

| Couche | Erreur | Stratégie |
|---|---|---|
| Classifier Gemini fail | 5xx/timeout | Escalation auto vers Sonnet, pas d'erreur user |
| Sonnet fail | 5xx/rate limit | Retry 2× exp backoff, fallback Haiku avec prompt simplifié |
| Anthropic quota | 429 | Message honnête : "service surchargé, réessaie dans 1 min" |
| Brave fail | 5xx | Retour vide au tool_result, Sonnet gère ("j'ai pas pu chercher") |
| Tool execution fail (D1 write) | error | Retry 1×, return error au tool_result, Sonnet dit "pas réussi" |
| RPC DO fail | après 3 retries | Rollback D1 + erreur claire à l'user |
| DO alarm fire alors action supprimée | action_id inexistant | Log, skip silencieusement |
| Vectorize fail | 5xx | query_chat_history renvoie "indisponible", Sonnet continue |
| Embedding Workers AI fail (ingest) | 5xx | Log + msg stocké en D1 sans embedding |
| iCal generate crash | RRULE bug | Skip l'item problématique, continue le .ics, log |
| Agent boucle 5 tours sans end_turn | max iterations | Coupe, envoie "Je n'ai pas réussi à finir, peux-tu préciser ?" |

### 17.3 Observabilité

- `console_log!` JSON structuré dans chaque erreur infra
- Compteurs KV : `errors:{type}:{YYYY-MM-DD}` par workspace
- Page admin `/admin/errors` (membres workspace "grumps-admin" only)
- CF Notifications sur Worker errors > 1% des requêtes

### 17.4 User-facing format

JSON API uniforme :
```jsonc
{
  "error": {
    "code": "QUOTA_EXCEEDED",
    "message": "Tu as atteint tes 5 recherches web gratuites ce mois-ci.",
    "details": { "used": 5, "limit": 5, "plan": "free" },
    "upgrade_url": "https://grumps.io/w/x7k9m2p4/billing"
  }
}
```

Côté SPA : hook `use_error_toast()` intercepte + affiche.

Côté chat, persona cohérente :
> *"Mauvaise journée pour les recherches — tu as utilisé tes 5 recherches gratuites ce mois-ci. Retour le 1er mai, ou plan Pro pour en avoir 50 → grumps.io/..."*

Jamais de stack trace leaké.

---

## 18. Routes API ajoutées

```
# Memory
GET    /api/w/:slug/memory                  # liste paginée + filtres
POST   /api/w/:slug/memory                  # création
GET    /api/w/:slug/memory/:id              # détail
PUT    /api/w/:slug/memory/:id              # update
DELETE /api/w/:slug/memory/:id              # suppression
POST   /api/w/:slug/memory/forget-me        # right to forget pour le sender
GET    /api/w/:slug/memory/export           # JSON RGPD

# Events
GET    /api/w/:slug/events
POST   /api/w/:slug/events
GET    /api/w/:slug/events/:id
PUT    /api/w/:slug/events/:id
DELETE /api/w/:slug/events/:id

# Calendar (vue agrégée)
GET    /api/w/:slug/calendar?from=&to=&members=&sources=

# iCal export
POST   /api/w/:slug/calendar/ical-token     # génération token (admin)
DELETE /api/w/:slug/calendar/ical-token     # révocation
GET    /cal/:slug.ics?t=<jwt>                # endpoint public

# Scheduled actions
GET    /api/w/:slug/scheduled
POST   /api/w/:slug/scheduled
GET    /api/w/:slug/scheduled/:id
PATCH  /api/w/:slug/scheduled/:id           # pause/resume/edit
DELETE /api/w/:slug/scheduled/:id
POST   /api/w/:slug/scheduled/:id/run-now   # exécution forcée

# Settings extensions
PUT    /api/w/:slug/settings                # extended (déjà existant, ajout des nouvelles clés)
```

---

## 19. Open questions / V2

### Open questions techniques

1. **Crate `rrule`** — vérifier compatibilité wasm32 ; sinon implémentation manuelle des cas usuels (~150 lignes, gérable)
2. **Workers AI bge-m3 disponibilité** — confirmer que le modèle est bien dispo dans Workers AI, sinon fallback sur `@cf/baai/bge-base-en-v1.5` (768-dim, anglais only) ou Voyage AI multilingue ($0.06/1M tokens)
3. **Workers RS Durable Objects + alarm** — valider la stabilité de `state.storage().set_alarm()` en production (la macro `#[durable_object]` est mature mais alarm est plus récente)
4. **Anthropic prompt cache** — confirmer que le caching marche bien avec notre structure de system prompt (memory pinned change rarement, devrait cacher OK)

### V2 / Évolutions futures

- Sync bidirectionnel Google Calendar (OAuth, conflits, quotas)
- Sync Outlook Calendar
- DM bot pour usage personnel
- Push notifications PWA + email digest
- Tools agentiques étendus (envoi email via Resend, intégrations API tierces)
- Recherche sémantique sur `memory_entries::value` (embeddings) — pour workspaces > 500 entries
- Mode (c) avec opt-in granulaire par sujet ("interviens si on parle de cuisine, pas si on parle de boulot")
- Marketplace de tools tiers
- Export complet workspace (RGPD), suppression complète

---

## 20. Décisions explicites validées (récap)

1. ✅ Mode proactif **configurable**, défaut semi-proactif (b), mode (c) opt-in Pro+
2. ✅ Mémoire **(1) structurée + (3) RAG** par défaut, **(2) auto-extraction** opt-in dès le lancement
3. ✅ Mémoire **strictement workspace-scoped** (silos étanches)
4. ✅ Actions schedulées : A + B + C + E (web search uniquement)
5. ✅ Calendrier 1/workspace, sync **iCal sortant** vers Google/Outlook (lecture seule), pas de sync bidirectionnel
6. ✅ Notifications **uniquement chat de groupe** (pas de DM/PWA/email)
7. ✅ Vision **complète développée d'un coup** (pas de phasage MVP)
8. ✅ Stack **100% Cloudflare** (Workers + D1 + KV + Vectorize + Workers AI + DOs)
9. ✅ LLM : Sonnet 4.6 cerveau agent + Gemini Flash classifier + Haiku fallback
10. ✅ Routing **cascade** : regex fast-path → Gemini classifier → CRUD direct OR Sonnet
11. ✅ Scheduling **DO + alarmes** (pas de polling continu, pas de cron de safety net, pas de cron de GC)
12. ✅ Web search : **Brave Base** par défaut, **SearXNG** OSS-ready en alternative
13. ✅ Calendrier UI : **warm brutalism** strict, mois + semaine + agenda, drag & drop, anti-AI-slop checklist
14. ✅ Design system préservé : slab serif, cream/brick/teal/slate, ombres dures, bordures épaisses
15. ✅ Pas de feature flags ni rollout progressif (pas en prod)

---

*Fin du spec. Prochaine étape : plan d'implémentation détaillé via `superpowers:writing-plans`.*
