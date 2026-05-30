// crates/worker/src/handler.rs
use grumps_messaging::adapter::OutboundMessage;
use grumps_messaging::formatter;
use grumps_nlu::parser::*;
use grumps_nlu::matcher;
use grumps_nlu::entity;
use grumps_nlu::llm::{NluIntent, NluResponse};
use crate::db::WorkspaceDb;
use crate::llm_client::LlmClient;
use worker::Env;
use grumps_agent::db::AgentDb as _;

pub struct HandlerResult {
    pub messages: Vec<OutboundMessage>,
}

impl HandlerResult {
    pub fn none() -> Self { Self { messages: vec![] } }
    pub fn one(text: String, reply_to: Option<String>) -> Self {
        Self { messages: vec![OutboundMessage { text, reply_to }] }
    }
    pub fn many(msgs: Vec<OutboundMessage>) -> Self { Self { messages: msgs } }
}

pub async fn handle_message(
    env: Option<&Env>,
    raw_text: &str,
    parse_result: ParseResult,
    inbound_message_id: &str,
    inbound_quoted_message_id: Option<&str>,
    inbound_quoted_message_text: Option<&str>,
    sender_name: &str,
    ws_db: &WorkspaceDb<'_>,
    member_id: &str,
    workspace_slug: &str,
    ws_locale: &str,
    llm_client: Option<&LlmClient>,
    ws_plan: &str,
) -> worker::Result<HandlerResult> {
    let locale = grumps_i18n::Locale::from_code(ws_locale);
    // Agent fast-path: route @grumps mentions through the agent before structured parsing.
    if let Some(env) = env {
        if let Some(result) = try_route_via_agent(env, ws_db, workspace_slug, member_id, raw_text, ws_locale).await? {
            return Ok(result);
        }
    }

    let plan = crate::billing::Plan::from_str(ws_plan);
    match parse_result {
        ParseResult::AddTodos(todos) => handle_add_todos(todos, inbound_message_id, ws_db, member_id, workspace_slug, locale, &plan).await,
        ParseResult::AddSingleTodo(todo) => {
            // If LLM client is available, classify free text instead of blindly creating a todo
            if let Some(llm) = llm_client {
                // Check LLM quota before making the call
                let llm_calls = ws_db.get_llm_calls_this_month().await.unwrap_or(0);
                if let Err(qe) = crate::billing::check_llm_quota(&plan, llm_calls) {
                    return Ok(HandlerResult::one(qe.render(locale), Some(inbound_message_id.to_string())));
                }

                let original_text = &todo.title;
                let open_todos = ws_db.get_open_todos().await?;
                let todo_pairs: Vec<(i64, String)> = open_todos.iter().map(|(_, title, seq)| (*seq, title.clone())).collect();

                // Anchor the NLU's relative-date resolution to the workspace's
                // local wall clock, so "tomorrow 9am" becomes a concrete time.
                let timezone = ws_db.get_setting("timezone").await.ok().flatten()
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "UTC".to_string());
                let tz: chrono_tz::Tz = timezone.parse().unwrap_or(chrono_tz::UTC);
                let now_local = chrono::Utc::now().with_timezone(&tz).format("%Y-%m-%dT%H:%M:%S").to_string();

                match llm.classify(original_text, sender_name, &todo_pairs, &now_local, &timezone).await {
                    Ok(nlu) => {
                        let _ = ws_db.increment_llm_calls().await;
                        return handle_llm_result(nlu, todo, inbound_message_id, ws_db, member_id, workspace_slug, locale, &plan, &timezone).await;
                    }
                    Err(e) => {
                        worker::console_log!("LLM classify error: {}, falling back to AddSingleTodo", e);
                    }
                }
            }
            handle_add_todos(vec![todo], inbound_message_id, ws_db, member_id, workspace_slug, locale, &plan).await
        }
        ParseResult::CompleteTodos(items) => handle_complete_todos(items, ws_db, member_id, inbound_message_id, locale).await,
        ParseResult::CompleteSingle(target) => handle_complete_single(target, ws_db, member_id, inbound_message_id, locale).await,
        ParseResult::DeleteTodo(seq) => handle_delete(seq, ws_db, member_id, locale).await,
        ParseResult::AddNote(note) => handle_add_note(note, ws_db, member_id, locale, &plan).await,
        ParseResult::ListTodos(filter) => handle_list_todos(filter, ws_db, member_id, locale).await,
        ParseResult::ListNotes => handle_list_notes(ws_db, locale).await,
        ParseResult::SearchNotes(query) => handle_search_notes(&query, ws_db, locale).await,
        ParseResult::ListFiles => Ok(HandlerResult::one(grumps_i18n::t(locale, "agent.files.web_only", &[]), None)),
        ParseResult::Help => Ok(HandlerResult::one(formatter::help_text(), None)),
        ParseResult::WorkspaceLink => Ok(HandlerResult::one(format!("grumps.app/w/{}", workspace_slug), None)),
        ParseResult::Status => handle_status(ws_db, workspace_slug, locale).await,
        ParseResult::QuotedTodo => handle_quoted_todo(inbound_quoted_message_text, inbound_message_id, ws_db, member_id, workspace_slug, locale, &plan).await,
        ParseResult::QuotedNote => handle_quoted_note(inbound_quoted_message_text, ws_db, member_id, locale, &plan).await,
        ParseResult::TaskCardReply(action) => handle_card_reply(action, inbound_quoted_message_id, inbound_message_id, ws_db, member_id, locale).await,
        ParseResult::Ignore => Ok(HandlerResult::none()),
    }
}

async fn handle_add_todos(todos: Vec<ParsedTodo>, msg_id: &str, ws_db: &WorkspaceDb<'_>, member_id: &str, slug: &str, locale: grumps_i18n::Locale, plan: &crate::billing::Plan) -> worker::Result<HandlerResult> {
    // Check todo quota before inserting
    let (open_count, _, _, _) = ws_db.get_status_counts().await?;
    if let Err(qe) = crate::billing::check_todo_quota(plan, open_count) {
        return Ok(HandlerResult::one(qe.render(locale), Some(msg_id.to_string())));
    }

    let mut messages = Vec::new();

    // Summary first
    messages.push(OutboundMessage {
        text: formatter::todos_added_summary(todos.len(), slug, locale.code()),
        reply_to: Some(msg_id.to_string()),
    });

    // Individual task card per todo
    for parsed in &todos {
        let tags_json = serde_json::to_string(&parsed.tags).unwrap_or_else(|_| "[]".into());
        let assignee = parsed.assignee_mention.as_deref().unwrap_or("");

        // Persist the deadline only when it's a real civil date (the LLM path
        // normalizes to YYYY-MM-DD; raw regex hints like "friday" fall through).
        let deadline = parsed.deadline_text.as_deref()
            .filter(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok());
        let (todo_id, seq) = ws_db.insert_todo(
            &parsed.title, parsed.priority.as_int(), &tags_json,
            assignee, assignee, member_id, "chat", msg_id, deadline,
        ).await?;

        ws_db.log_activity(member_id, "todo.created", "todo", &todo_id, "chat").await?;

        let card = formatter::task_card(
            seq, &parsed.title,
            parsed.assignee_mention.as_deref(),
            parsed.deadline_text.as_deref(),
            parsed.priority,
            &parsed.tags,
            locale.code(),
        );
        messages.push(OutboundMessage { text: card, reply_to: None });
    }

    Ok(HandlerResult::many(messages))
}

async fn handle_complete_todos(items: Vec<String>, ws_db: &WorkspaceDb<'_>, member_id: &str, msg_id: &str, locale: grumps_i18n::Locale) -> worker::Result<HandlerResult> {
    let open_todos = ws_db.get_open_todos().await?;
    let mut lines = Vec::new();

    for item in &items {
        match matcher::match_done(item, &open_todos) {
            matcher::MatchResult::Exact(m) => {
                ws_db.complete_todo(&m.todo_id, member_id).await?;
                ws_db.log_activity(member_id, "todo.completed", "todo", &m.todo_id, "chat").await?;
                let seq_str = m.seq_num.to_string();
                let mut line = grumps_i18n::t(locale, "agent.todo.completed",
                    &[("seq", &seq_str), ("title", &m.title)]);
                if let Ok(Some(rec)) = ws_db.get_todo_recurrence(&m.todo_id).await {
                    if let Ok(Some(todo)) = ws_db.get_todo_by_seq(m.seq_num).await {
                        let _ = ws_db.create_next_recurrence(&todo, &rec).await;
                        line.push_str(&grumps_i18n::t(locale, "agent.card.next_occurrence", &[]));
                    }
                }
                lines.push(line);
            }
            matcher::MatchResult::Fuzzy(candidates) => {
                let opts: Vec<String> = candidates.iter().enumerate()
                    .map(|(i, c)| format!("  {}. #{} \"{}\"", i + 1, c.seq_num, c.title)).collect();
                let n_str = candidates.len().to_string();
                let header = grumps_i18n::t(locale, "agent.todo.fuzzy_match_header",
                    &[("item", item.as_str()), ("n", &n_str)]);
                let footer = grumps_i18n::t(locale, "agent.todo.fuzzy_match_footer", &[]);
                lines.push(format!("{}\n{}\n{}", header, opts.join("\n"), footer));
            }
            matcher::MatchResult::NoMatch => {
                lines.push(grumps_i18n::t(locale, "agent.todo.no_match", &[("item", item.as_str())]));
            }
        }
    }

    Ok(HandlerResult::one(lines.join("\n\n"), Some(msg_id.to_string())))
}

async fn handle_complete_single(target: CompletionTarget, ws_db: &WorkspaceDb<'_>, member_id: &str, msg_id: &str, locale: grumps_i18n::Locale) -> worker::Result<HandlerResult> {
    match target {
        CompletionTarget::BySeqNum(seq) => {
            let seq_str = seq.to_string();
            match ws_db.get_todo_by_seq(seq).await? {
                Some(todo) => {
                    ws_db.complete_todo(&todo.id, member_id).await?;
                    ws_db.log_activity(member_id, "todo.completed", "todo", &todo.id, "chat").await?;
                    let mut text = grumps_i18n::t(locale, "agent.todo.completed",
                        &[("seq", &seq_str), ("title", &todo.title)]);
                    if let Some(ref rec) = todo.recurrence {
                        if !rec.is_empty() {
                            let _ = ws_db.create_next_recurrence(&todo, rec).await;
                            text.push_str(&grumps_i18n::t(locale, "agent.card.next_occurrence", &[]));
                        }
                    }
                    Ok(HandlerResult::one(text, Some(msg_id.to_string())))
                }
                None => Ok(HandlerResult::one(
                    grumps_i18n::t(locale, "agent.todo.not_found", &[("seq", &seq_str)]),
                    Some(msg_id.to_string()),
                )),
            }
        }
        CompletionTarget::ByText(text) => handle_complete_todos(vec![text], ws_db, member_id, msg_id, locale).await,
    }
}

async fn handle_delete(seq: i64, ws_db: &WorkspaceDb<'_>, member_id: &str, locale: grumps_i18n::Locale) -> worker::Result<HandlerResult> {
    let seq_str = seq.to_string();
    match ws_db.get_todo_by_seq(seq).await? {
        Some(todo) => {
            ws_db.delete_todo(&todo.id).await?;
            ws_db.log_activity(member_id, "todo.deleted", "todo", &todo.id, "chat").await?;
            Ok(HandlerResult::one(grumps_i18n::t(locale, "agent.todo.deleted",
                &[("seq", &seq_str), ("title", &todo.title)]), None))
        }
        None => Ok(HandlerResult::one(grumps_i18n::t(locale, "agent.todo.not_found",
            &[("seq", &seq_str)]), None)),
    }
}

async fn handle_add_note(note: ParsedNote, ws_db: &WorkspaceDb<'_>, member_id: &str, locale: grumps_i18n::Locale, plan: &crate::billing::Plan) -> worker::Result<HandlerResult> {
    // Check note quota
    let (_, _, note_count, _) = ws_db.get_status_counts().await?;
    if let Err(qe) = crate::billing::check_note_quota(plan, note_count) {
        return Ok(HandlerResult::one(qe.render(locale), None));
    }

    let title = note.title.as_deref().unwrap_or("");
    let note_id = ws_db.insert_note(title, &note.content, "chat", member_id).await?;
    ws_db.log_activity(member_id, "note.created", "note", &note_id, "chat").await?;
    let t = note.title.map(|t| format!(" \"{}\"", t)).unwrap_or_default();
    Ok(HandlerResult::one(grumps_i18n::t(locale, "agent.note.saved", &[("title", &t)]), None))
}

async fn handle_list_todos(filter: ListFilter, ws_db: &WorkspaceDb<'_>, member_id: &str, locale: grumps_i18n::Locale) -> worker::Result<HandlerResult> {
    let (actual_filter, actual_label): (String, String) = match &filter {
        ListFilter::Open => ("open".to_string(), "open".to_string()),
        ListFilter::All => ("all".to_string(), "all".to_string()),
        ListFilter::Mine => ("mine".to_string(), "mine".to_string()),
        ListFilter::Done => ("done".to_string(), "done".to_string()),
        ListFilter::Assignee(name) => (format!("assignee:{}", name), format!("@{}", name)),
        ListFilter::Tag(tag) => (format!("tag:{}", tag), format!("#{}", tag)),
    };

    let todos = ws_db.get_todos_filtered(&actual_filter, Some(member_id)).await?;
    Ok(HandlerResult::one(formatter::todo_list(&todos, &actual_label, locale.code()), None))
}

async fn handle_list_notes(ws_db: &WorkspaceDb<'_>, locale: grumps_i18n::Locale) -> worker::Result<HandlerResult> {
    let notes = ws_db.get_notes().await?;
    Ok(HandlerResult::one(formatter::note_list(&notes, locale.code()), None))
}

async fn handle_search_notes(query: &str, ws_db: &WorkspaceDb<'_>, locale: grumps_i18n::Locale) -> worker::Result<HandlerResult> {
    let notes = ws_db.search_notes(query).await?;
    if notes.is_empty() {
        Ok(HandlerResult::one(grumps_i18n::t(locale, "agent.notes.search_empty", &[("query", query)]), None))
    } else {
        Ok(HandlerResult::one(formatter::note_list(&notes, locale.code()), None))
    }
}

async fn handle_status(ws_db: &WorkspaceDb<'_>, slug: &str, locale: grumps_i18n::Locale) -> worker::Result<HandlerResult> {
    let (open, done_week, notes, files) = ws_db.get_status_counts().await?;
    Ok(HandlerResult::one(formatter::status_summary(open, done_week, notes, files, slug, locale.code()), None))
}

async fn handle_quoted_todo(quoted_text: Option<&str>, msg_id: &str, ws_db: &WorkspaceDb<'_>, member_id: &str, slug: &str, locale: grumps_i18n::Locale, plan: &crate::billing::Plan) -> worker::Result<HandlerResult> {
    let content = quoted_text.unwrap_or("");
    if content.is_empty() {
        return Ok(HandlerResult::one(grumps_i18n::t(locale, "agent.quoted.no_todo", &[]), None));
    }
    let parsed = entity::extract_todo_from_line(content);
    handle_add_todos(vec![parsed], msg_id, ws_db, member_id, slug, locale, plan).await
}

async fn handle_quoted_note(quoted_text: Option<&str>, ws_db: &WorkspaceDb<'_>, member_id: &str, locale: grumps_i18n::Locale, plan: &crate::billing::Plan) -> worker::Result<HandlerResult> {
    let content = quoted_text.unwrap_or("");
    if content.is_empty() {
        return Ok(HandlerResult::one(grumps_i18n::t(locale, "agent.quoted.no_note", &[]), None));
    }
    let note = ParsedNote { title: None, content: content.to_string() };
    handle_add_note(note, ws_db, member_id, locale, plan).await
}

async fn handle_card_reply(action: TaskCardAction, quoted_msg_id: Option<&str>, msg_id: &str, ws_db: &WorkspaceDb<'_>, member_id: &str, locale: grumps_i18n::Locale) -> worker::Result<HandlerResult> {
    // Look up which todo this reply refers to
    let todo_id = if let Some(qid) = quoted_msg_id {
        ws_db.get_todo_for_bot_message(qid).await.unwrap_or(None)
    } else { None };

    let text = match action {
        TaskCardAction::Done => {
            let mut text = grumps_i18n::t(locale, "agent.card.done", &[]);
            if let Some(ref tid) = todo_id {
                ws_db.complete_todo(tid, member_id).await?;
                ws_db.log_activity(member_id, "todo.completed", "todo", tid, "chat").await?;
                if let Ok(Some(todo)) = ws_db.get_todo_by_id(tid).await {
                    if let Some(ref rec) = todo.recurrence {
                        if !rec.is_empty() {
                            let _ = ws_db.create_next_recurrence(&todo, rec).await;
                            text.push_str(&grumps_i18n::t(locale, "agent.card.next_occurrence", &[]));
                        }
                    }
                }
            }
            text
        }
        TaskCardAction::Delete => {
            if let Some(ref tid) = todo_id {
                ws_db.delete_todo(tid).await?;
                ws_db.log_activity(member_id, "todo.deleted", "todo", tid, "chat").await?;
            }
            grumps_i18n::t(locale, "agent.card.deleted", &[])
        }
        TaskCardAction::Snooze(time) => grumps_i18n::t(locale, "agent.card.snoozed", &[("time", &time)]),
        TaskCardAction::Edit(title) => grumps_i18n::t(locale, "agent.card.updated", &[("title", &title)]),
        TaskCardAction::Reassign(person) => grumps_i18n::t(locale, "agent.card.reassigned", &[("person", &person)]),
        TaskCardAction::ChangePriority(p) => grumps_i18n::t(locale, "agent.card.priority", &[("emoji", p.emoji()), ("label", p.label())]),
        TaskCardAction::AddTag(tag) => grumps_i18n::t(locale, "agent.card.tag_added", &[("tag", &tag)]),
        TaskCardAction::ChangeStatus(s) => grumps_i18n::t(locale, "agent.card.status", &[("status", &s)]),
    };

    Ok(HandlerResult::one(text, Some(msg_id.to_string())))
}

async fn try_route_via_agent(
    env: &Env,
    ws_db: &WorkspaceDb<'_>,
    ws_slug: &str,
    member_id: &str,
    text: &str,
    ws_locale: &str,
) -> worker::Result<Option<HandlerResult>> {
    // Only route via agent if the message contains an @grumps mention.
    let lower = text.to_lowercase();
    let has_mention = lower.contains("@grumps") || lower.contains("@heygrumpsbot");
    if !has_mention {
        return Ok(None);
    }

    // Fast-path: silence / unsilence / explicit memory commands
    if let Some(result) = try_fast_commands(env, ws_db, ws_slug, member_id, &lower, text).await? {
        return Ok(Some(result));
    }

    // Skip if it's a structured command already handled by the fast-path.
    let trimmed_upper = text.trim_start().to_uppercase();
    if trimmed_upper.starts_with("TODO:")
        || trimmed_upper.starts_with("DONE:")
        || trimmed_upper.starts_with("NOTE:")
        || trimmed_upper.starts_with("REMIND:")
    {
        return Ok(None);
    }

    let sink = crate::agent_sink::WorkerMessagingSink {
        env,
        ws_slug: ws_slug.to_string(),
    };

    let has_session = ws_db
        .get_active_agent_session(member_id)
        .await
        .ok()
        .flatten()
        .is_some();

    let route_result = grumps_agent::router::route_message(
        env,
        ws_slug,
        member_id,
        text,
        has_session,
        &sink,
        ws_db,
        ws_locale,
    )
    .await;

    match route_result {
        Ok(_) => {
            // sink.send already pushed the message — return empty HandlerResult to avoid double-send.
            Ok(Some(HandlerResult::none()))
        }
        Err(e) => {
            worker::console_log!("agent route failed for ws={ws_slug}: {e}");
            // Resolve workspace locale; fall back to En on lookup failure.
            let locale = match crate::db::get_index_db(env)
                .ok()
                .and_then(|db| Some((db, ws_slug)))
            {
                Some((db, slug)) => match crate::db::lookup_workspace_by_slug(&db, slug).await {
                    Ok(Some(ws)) => grumps_i18n::Locale::from_code(&ws.locale),
                    _ => grumps_i18n::Locale::En,
                },
                None => grumps_i18n::Locale::En,
            };
            Ok(Some(HandlerResult::one(
                grumps_i18n::t(locale, "agent.error.technical", &[]),
                None,
            )))
        }
    }
}

/// Fast-path for silence/unsilence/explicit-memory commands addressed to @grumps.
/// `lower` is the lowercased version of the full text; `text` is the original.
async fn try_fast_commands(
    env: &Env,
    ws_db: &WorkspaceDb<'_>,
    ws_slug: &str,
    member_id: &str,
    lower: &str,
    text: &str,
) -> worker::Result<Option<HandlerResult>> {
    let kv = match env.kv("KV") {
        Ok(k) => k,
        Err(_) => return Ok(None),
    };

    // Resolve the workspace's locale for response strings. Falls back to En
    // on any error so the fast-path stays available even if the index DB is
    // briefly unreachable.
    let locale = match crate::db::get_index_db(env)
        .ok()
        .and_then(|db| Some((db, ws_slug)))
    {
        Some((db, slug)) => match crate::db::lookup_workspace_by_slug(&db, slug).await {
            Ok(Some(ws)) => grumps_i18n::Locale::from_code(&ws.locale),
            _ => grumps_i18n::Locale::En,
        },
        None => grumps_i18n::Locale::En,
    };

    // Silence: matched in every supported language plus a few common synonyms.
    let silence_triggers = [
        "@grumps tais-toi", "@grumps quiet", "@grumps silence", "@grumps shh",
        "@grumps cállate", "@grumps cala-te", "@grumps cale-se",
        "@grumps ruhe", "@grumps zitto", "@grumps тише", "@grumps sus",
        "@grumps اسكت", "@grumps चुप", "@grumps 安静", "@grumps 静かに",
        "@grumps 조용히", "@grumps diam",
    ];
    if silence_triggers.iter().any(|t| lower.contains(t)) {
        let silence_key = format!("proactive:{ws_slug}:silence_until");
        let _ = kv.put(&silence_key, "1").map(|p| p.expiration_ttl(86400).execute());
        return Ok(Some(HandlerResult::one(
            grumps_i18n::t(locale, "agent.silence.confirm", &[]),
            None,
        )));
    }

    let unsilence_triggers = [
        "@grumps reviens", "@grumps unquiet", "@grumps come back",
        "@grumps vuelve", "@grumps volta",
        "@grumps zurück", "@grumps torna", "@grumps вернись", "@grumps geri dön",
        "@grumps عُد", "@grumps वापस आओ", "@grumps 回来", "@grumps 戻って",
        "@grumps 돌아와", "@grumps kembali",
    ];
    if unsilence_triggers.iter().any(|t| lower.contains(t)) {
        let silence_key = format!("proactive:{ws_slug}:silence_until");
        let _ = kv.delete(&silence_key).await;
        return Ok(Some(HandlerResult::one(
            grumps_i18n::t(locale, "agent.silence.unset", &[]),
            None,
        )));
    }

    // Explicit memory: works in fr ("souviens-toi que/de") and en
    // ("remember that"). Picks whichever phrase the user happened to use.
    let memory_phrases: &[&str] = &[
        "@grumps souviens-toi que ",
        "@grumps souviens-toi de ",
        "@grumps remember that ",
        "@grumps remember to ",
        "@grumps note that ",
    ];
    let memory_trigger = memory_phrases.iter()
        .find_map(|phrase| lower.find(phrase).map(|i| i + phrase.len()));

    if let Some(start) = memory_trigger {
        // Extract the content from the original text (preserving case), same start offset
        let content = text[start..].trim().to_string();
        if !content.is_empty() {
            let entry = grumps_memory::NewMemoryEntry {
                key: None,
                value: content.clone(),
                kind: grumps_memory::MemoryKind::Other,
                related_member: None,
                tags: vec![],
                source: grumps_memory::MemorySource::ChatExplicit,
                confidence: Some(1.0),
                pinned: Some(false),
                expires_at: None,
                created_by: Some(member_id.to_string()),
            };
            match ws_db.create_memory(&entry).await {
                Ok(_) => return Ok(Some(HandlerResult::one(
                    grumps_i18n::t(locale, "agent.memory.saved", &[]),
                    None,
                ))),
                Err(e) => {
                    worker::console_log!("fast-path create_memory failed: {e}");
                    return Ok(Some(HandlerResult::one(
                        grumps_i18n::t(locale, "agent.memory.error", &[]),
                        None,
                    )));
                }
            }
        }
    }

    Ok(None)
}

async fn handle_llm_result(
    nlu: NluResponse,
    original_todo: ParsedTodo,
    msg_id: &str,
    ws_db: &WorkspaceDb<'_>,
    member_id: &str,
    slug: &str,
    locale: grumps_i18n::Locale,
    plan: &crate::billing::Plan,
    timezone: &str,
) -> worker::Result<HandlerResult> {
    use grumps_core::todo::Priority;

    match nlu.intent {
        NluIntent::AddTodo => {
            // Use LLM-extracted entities to enrich the todo
            let mut todo = original_todo;
            if let Some(title) = nlu.entities.title {
                todo.title = title;
            }
            if let Some(assignee) = nlu.entities.assignee {
                todo.assignee_mention = Some(assignee);
            }
            if let Some(deadline) = nlu.entities.deadline {
                // Normalize to a civil date (YYYY-MM-DD) in the workspace tz; a
                // deadline is a calendar day, not an instant. Unparseable → no
                // deadline (better than storing relative text that never matches).
                let tz: chrono_tz::Tz = timezone.parse().unwrap_or(chrono_tz::UTC);
                todo.deadline_text = grumps_agent::tools::parse_user_date(&deadline, &tz);
            }
            if let Some(ref p) = nlu.entities.priority {
                todo.priority = match p.as_str() {
                    "high" => Priority::High,
                    "low" => Priority::Low,
                    _ => Priority::Normal,
                };
            }
            if !nlu.entities.tags.is_empty() {
                todo.tags = nlu.entities.tags;
            }
            handle_add_todos(vec![todo], msg_id, ws_db, member_id, slug, locale, plan).await
        }
        NluIntent::CompleteTodo => {
            if let Some(seq) = nlu.entities.target_id {
                handle_complete_single(CompletionTarget::BySeqNum(seq), ws_db, member_id, msg_id, locale).await
            } else if let Some(title) = nlu.entities.title {
                handle_complete_single(CompletionTarget::ByText(title), ws_db, member_id, msg_id, locale).await
            } else {
                // Fall back to treating original text as completion target
                handle_complete_todos(vec![original_todo.title], ws_db, member_id, msg_id, locale).await
            }
        }
        NluIntent::AddNote => {
            let note = ParsedNote {
                title: nlu.entities.title.clone(),
                content: nlu.entities.title.unwrap_or(original_todo.title),
            };
            handle_add_note(note, ws_db, member_id, locale, plan).await
        }
        NluIntent::SetReminder => {
            let tz: chrono_tz::Tz = timezone.parse().unwrap_or(chrono_tz::UTC);
            // Resolve the model's concrete local datetime to a UTC instant. If
            // it's missing or unparseable, ask for a time rather than store a
            // reminder that can never fire (datetime(NULL) never matches).
            let remind_at_utc = match nlu.entities.deadline.as_deref()
                .and_then(|d| grumps_agent::tools::parse_user_datetime(d, &tz))
            {
                Some(dt) => dt,
                None => return Ok(HandlerResult::one(
                    grumps_i18n::t(locale, "agent.reminder.need_time", &[]),
                    Some(msg_id.to_string()),
                )),
            };
            // Store UTC (Z); display the local wall clock the user expects.
            let remind_at = remind_at_utc.format("%Y-%m-%dT%H:%M:%SZ").to_string();
            let remind_at_display = remind_at_utc.with_timezone(&tz).format("%Y-%m-%d %H:%M").to_string();

            let default_title = grumps_i18n::t(locale, "agent.reminder.default_title", &[]);
            let title = nlu.entities.title.unwrap_or(default_title);
            let target = nlu.entities.assignee.as_deref().unwrap_or(member_id);
            let recurrence = nlu.entities.recurrence.as_deref();

            let id = ws_db.insert_reminder(&title, &remind_at, recurrence, target, member_id).await?;
            ws_db.log_activity(member_id, "reminder.created", "reminder", &id, "chat").await?;

            let rec_text = recurrence.map(|r| format!(" ({})", r)).unwrap_or_default();
            let target_text = if target != member_id {
                grumps_i18n::t(locale, "agent.reminder.target_for",
                    &[("name", nlu.entities.assignee.as_deref().unwrap_or(""))])
            } else {
                String::new()
            };

            Ok(HandlerResult::one(
                grumps_i18n::t(locale, "agent.reminder.set", &[
                    ("target", &target_text),
                    ("title", &title),
                    ("remind_at", &remind_at_display),
                    ("rec", &rec_text),
                ]),
                Some(msg_id.to_string()),
            ))
        }
        NluIntent::ListTodos => {
            handle_list_todos(ListFilter::Open, ws_db, member_id, locale).await
        }
        NluIntent::ListNotes => {
            handle_list_notes(ws_db, locale).await
        }
        NluIntent::SearchNotes => {
            let query = nlu.entities.search_query.unwrap_or(original_todo.title);
            handle_search_notes(&query, ws_db, locale).await
        }
        NluIntent::DeleteTodo => {
            if let Some(seq) = nlu.entities.target_id {
                handle_delete(seq, ws_db, member_id, locale).await
            } else {
                Ok(HandlerResult::one(grumps_i18n::t(locale, "agent.todo.which_to_delete", &[]), Some(msg_id.to_string())))
            }
        }
        NluIntent::Summarize => {
            Ok(HandlerResult::one(grumps_i18n::t(locale, "agent.summarize.coming_soon", &[]), Some(msg_id.to_string())))
        }
        NluIntent::Help => {
            Ok(HandlerResult::one(grumps_messaging::formatter::help_text(), None))
        }
        NluIntent::Status => {
            handle_status(ws_db, slug, locale).await
        }
        NluIntent::Irrelevant => {
            Ok(HandlerResult::none())
        }
    }
}
