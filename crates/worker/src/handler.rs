// crates/worker/src/handler.rs
use crate::db::WorkspaceDb;
use crate::magic_link;
use chrono::Datelike;
use grumps_messaging::adapter::OutboundMessage;
use grumps_messaging::formatter;
use grumps_nlu::entity;
use grumps_nlu::matcher;
use grumps_nlu::parser::*;
use worker::Env;

pub struct HandlerResult {
    pub messages: Vec<OutboundMessage>,
}

impl HandlerResult {
    pub fn none() -> Self {
        Self { messages: vec![] }
    }
    pub fn one(text: String, reply_to: Option<String>) -> Self {
        Self {
            messages: vec![OutboundMessage {
                text,
                reply_to,
                todo_id: None,
                markdown: false,
            }],
        }
    }
    /// Like [`Self::one`], but flags the message as containing intentional
    /// `*bold*`/`_italic_` markup (currently only `help_text`) so the
    /// Telegram adapter renders it with `parse_mode: Markdown`.
    pub fn one_markdown(text: String, reply_to: Option<String>) -> Self {
        Self {
            messages: vec![OutboundMessage {
                text,
                reply_to,
                todo_id: None,
                markdown: true,
            }],
        }
    }
    pub fn many(msgs: Vec<OutboundMessage>) -> Self {
        Self { messages: msgs }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_message(
    env: Option<&Env>,
    raw_text: &str,
    parse_result: ParseResult,
    inbound_message_id: &str,
    inbound_quoted_message_id: Option<&str>,
    inbound_quoted_message_text: Option<&str>,
    ws_db: &WorkspaceDb<'_>,
    member_id: &str,
    workspace_slug: &str,
    ws_locale: &str,
    ws_plan: &str,
    // Below: only consumed by the `WorkspaceLink` arm (magic-link mint +
    // DM-only delivery for platforms without a web login widget). Threaded
    // through the full signature rather than re-deriving from `ws_db`
    // because `sender_id`/`is_direct_message` are per-message inbound
    // fields, not workspace state.
    platform: &str,
    sender_id: &str,
    is_direct_message: bool,
) -> worker::Result<HandlerResult> {
    let locale = grumps_i18n::Locale::from_code(ws_locale);
    // Explicit @grumps directives (silence / unsilence / memory) are handled
    // deterministically before anything else. Everything else goes through the
    // parser first (deterministic commands), with the agent as a last resort
    // for unrecognized free text — see the `Freeform` arm below.
    if let Some(env) = env {
        let lower = raw_text.to_lowercase();
        if lower.contains("@grumps") || lower.contains("@heygrumpsbot") {
            if let Some(result) =
                try_fast_commands(env, ws_db, workspace_slug, member_id, raw_text).await?
            {
                return Ok(result);
            }
        }
    }

    let plan = crate::billing::Plan::from_str(ws_plan);
    match parse_result {
        ParseResult::AddTodos(todos) => {
            handle_add_todos(
                todos,
                inbound_message_id,
                ws_db,
                member_id,
                workspace_slug,
                locale,
                &plan,
            )
            .await
        }
        ParseResult::Freeform(text) => {
            // Last resort: unrecognized free text goes to the LLM agent, which
            // classifies intent, may run tools, and replies via the sink.
            if let Some(env) = env {
                return route_via_agent(env, ws_db, workspace_slug, member_id, &text, ws_locale)
                    .await;
            }
            // No agent available (e.g. unit tests): create a single todo from
            // the text.
            let todo = entity::extract_todo_from_line(&text);
            handle_add_todos(
                vec![todo],
                inbound_message_id,
                ws_db,
                member_id,
                workspace_slug,
                locale,
                &plan,
            )
            .await
        }
        ParseResult::CompleteTodos(items) => {
            handle_complete_todos(
                items,
                ws_db,
                member_id,
                inbound_message_id,
                workspace_slug,
                locale,
            )
            .await
        }
        ParseResult::CompleteSingle(target) => {
            handle_complete_single(
                target,
                ws_db,
                member_id,
                inbound_message_id,
                workspace_slug,
                locale,
            )
            .await
        }
        ParseResult::DeleteTodo(seq) => handle_delete(seq, ws_db, member_id, locale).await,
        ParseResult::AddNote(note) => handle_add_note(note, ws_db, member_id, locale, &plan).await,
        ParseResult::ListTodos(filter) => handle_list_todos(filter, ws_db, member_id, locale).await,
        ParseResult::ListNotes => handle_list_notes(ws_db, locale).await,
        ParseResult::SearchNotes(query) => handle_search_notes(&query, ws_db, locale).await,
        ParseResult::Help => Ok(HandlerResult::one_markdown(
            formatter::help_text(locale.code()),
            None,
        )),
        ParseResult::WorkspaceLink => {
            magic_link::handle_workspace_link(
                env,
                member_id,
                workspace_slug,
                sender_id,
                platform,
                is_direct_message,
                locale,
            )
            .await
        }
        ParseResult::Status => handle_status(ws_db, workspace_slug, locale).await,
        ParseResult::QuotedTodo => {
            handle_quoted_todo(
                inbound_quoted_message_text,
                inbound_message_id,
                ws_db,
                member_id,
                workspace_slug,
                locale,
                &plan,
            )
            .await
        }
        ParseResult::QuotedNote => {
            handle_quoted_note(inbound_quoted_message_text, ws_db, member_id, locale, &plan).await
        }
        ParseResult::TaskCardReply(action) => {
            handle_card_reply(
                action,
                inbound_quoted_message_id,
                inbound_message_id,
                ws_db,
                member_id,
                workspace_slug,
                locale,
            )
            .await
        }
        ParseResult::Ignore => Ok(HandlerResult::none()),
    }
}

async fn handle_add_todos(
    todos: Vec<ParsedTodo>,
    msg_id: &str,
    ws_db: &WorkspaceDb<'_>,
    member_id: &str,
    slug: &str,
    locale: grumps_i18n::Locale,
    plan: &crate::billing::Plan,
) -> worker::Result<HandlerResult> {
    // Enforce the todo quota per item inside the batch: a single
    // `TODO: a\nTODO: b\n...` message can request more todos than the
    // remaining quota allows. `todos_batch_allowance` computes how many of
    // the requested todos actually fit (pure — see grumps_core::billing),
    // so only the first `allowed` get inserted below and the existing quota
    // message is appended for the rest, instead of a single all-or-nothing
    // check against the whole batch. Only `open` is used here, so the
    // (tz-sensitive) "this week" count is irrelevant — pass UTC to skip a read.
    let (open_count, _, _, _) = ws_db.get_status_counts("UTC").await?;
    let allowed = plan.todos_batch_allowance(open_count, todos.len());
    if allowed == 0 {
        return Ok(match crate::billing::check_todo_quota(plan, open_count) {
            Err(qe) => HandlerResult::one(qe.render(locale), Some(msg_id.to_string())),
            // Only reachable if `todos` was empty to begin with.
            Ok(()) => HandlerResult::none(),
        });
    }

    let mut messages = Vec::new();

    // Single todo: skip the separate summary and fold the workspace link into
    // the one card instead — two messages for one todo is noisy. Multiple
    // todos keep the summary-then-cards layout. "Single" reflects what's
    // actually being created (`allowed`), not what was requested — a batch
    // truncated down to one item still gets the single-card treatment.
    let single = allowed == 1;
    if !single {
        messages.push(OutboundMessage {
            text: formatter::todos_added_summary(allowed, slug, locale.code()),
            reply_to: Some(msg_id.to_string()),
            todo_id: None,
            markdown: false,
        });
    }

    // Relative deadline hints ("friday", "demain", ...) are resolved to a real
    // civil date in the workspace timezone *before* insert, so what the card
    // displays is exactly what gets stored (see resolve_relative_date).
    let tz = workspace_tz(ws_db).await;
    let today = grumps_core::timeutil::today_in_tz(tz);

    // Degressive hint + once-a-day link, computed once for the whole add
    // (single card or batch) — see `card_chrome`. Multi-todo adds: the
    // summary above already carries the workspace link unconditionally, and
    // `card_chrome`'s side effect marks today's link as consumed, so every
    // per-card link is suppressed; only the first card of the batch may
    // show the reply hint.
    let (show_hint, show_link) = card_chrome(ws_db, tz).await;

    // Individual task card per todo — only the first `allowed` (quota-limited
    // above) are inserted.
    for (idx, parsed) in todos.iter().take(allowed).enumerate() {
        let tags_json = serde_json::to_string(&parsed.tags).unwrap_or_else(|_| "[]".into());
        let assignee = parsed.assignee_mention.as_deref().unwrap_or("");

        // Resolve the raw hint into the value that gets stored: an explicit
        // YYYY-MM-DD (the LLM path already normalizes to this) passes through
        // `resolve_relative_date`'s ISO branch; "friday"/"demain"/... resolve
        // via the shared nlu resolver. Anything else stays unresolved — not
        // persisted, but still shown on the card as raw text (prior behavior).
        let resolved_deadline = parsed
            .deadline_text
            .as_deref()
            .and_then(|d| entity::resolve_relative_date(d, today))
            .map(|d| d.format("%Y-%m-%d").to_string());
        let deadline = resolved_deadline.as_deref();
        let display_deadline = resolved_deadline
            .as_deref()
            .or(parsed.deadline_text.as_deref());

        let (todo_id, seq) = ws_db
            .insert_todo(
                &parsed.title,
                parsed.priority.as_int(),
                &tags_json,
                assignee,
                assignee,
                member_id,
                "chat",
                msg_id,
                deadline,
            )
            .await?;

        ws_db
            .log_activity(member_id, "todo.created", "todo", &todo_id, "chat")
            .await?;

        let (deadline_display, _, _) = deadline_display_info(locale, display_deadline, today);
        let card_hint = show_hint && idx == 0;
        let card_link = single && show_link;
        let card = formatter::task_card(
            seq,
            &parsed.title,
            parsed.assignee_mention.as_deref(),
            deadline_display.as_deref(),
            parsed.priority,
            &parsed.tags,
            false, // the deterministic `TODO:` parser doesn't support recurrence yet
            card_hint,
            if card_link { Some(slug) } else { None },
        );
        // Single todo: no separate summary message, so the card is anchored
        // to the user's message directly.
        let reply_to = if single {
            Some(msg_id.to_string())
        } else {
            None
        };
        // Carry the todo id so the webhook send loop records it against the
        // sent card message — a later reply to the card then resolves to this
        // todo (see handle_card_reply / track_bot_message).
        messages.push(OutboundMessage {
            text: card,
            reply_to,
            todo_id: Some(todo_id.clone()),
            markdown: false,
        });
    }

    // The batch was truncated by quota — tell the group why the remaining
    // lines were dropped, using the same quota message a single-item hit
    // would show.
    if allowed < todos.len() {
        if let Err(qe) = crate::billing::check_todo_quota(plan, open_count + allowed as i64) {
            messages.push(OutboundMessage {
                text: qe.render(locale),
                reply_to: None,
                todo_id: None,
                markdown: false,
            });
        }
    }

    Ok(HandlerResult::many(messages))
}

async fn handle_complete_todos(
    items: Vec<String>,
    ws_db: &WorkspaceDb<'_>,
    member_id: &str,
    msg_id: &str,
    slug: &str,
    locale: grumps_i18n::Locale,
) -> worker::Result<HandlerResult> {
    let open_todos = ws_db.get_open_todos().await?;
    let mut lines = Vec::new();
    let mut extra_cards = Vec::new();

    for item in &items {
        match matcher::match_done(item, &open_todos) {
            matcher::MatchResult::Exact(m) => {
                ws_db.complete_todo(&m.todo_id, member_id).await?;
                ws_db
                    .log_activity(member_id, "todo.completed", "todo", &m.todo_id, "chat")
                    .await?;
                let seq_str = m.seq_num.to_string();
                let mut line = grumps_i18n::t(
                    locale,
                    "agent.todo.completed",
                    &[("seq", &seq_str), ("title", &m.title)],
                );
                if let Ok(Some(rec)) = ws_db.get_todo_recurrence(&m.todo_id).await {
                    if let Ok(Some(todo)) = ws_db.get_todo_by_seq(m.seq_num).await {
                        let tz = workspace_tz(ws_db).await;
                        if let Ok(next) = ws_db.create_next_recurrence(&todo, &rec, tz).await {
                            line.push_str(&grumps_i18n::t(
                                locale,
                                "agent.card.next_occurrence",
                                &[],
                            ));
                            extra_cards
                                .push(next_occurrence_card(ws_db, slug, &next, locale, tz).await);
                        }
                    }
                }
                lines.push(line);
            }
            matcher::MatchResult::Fuzzy(candidates) => {
                let opts: Vec<String> = candidates
                    .iter()
                    .enumerate()
                    .map(|(i, c)| format!("  {}. #{} \"{}\"", i + 1, c.seq_num, c.title))
                    .collect();
                let n_str = candidates.len().to_string();
                let header = grumps_i18n::t(
                    locale,
                    "agent.todo.fuzzy_match_header",
                    &[("item", item.as_str()), ("n", &n_str)],
                );
                let footer = grumps_i18n::t(locale, "agent.todo.fuzzy_match_footer", &[]);
                lines.push(format!("{}\n{}\n{}", header, opts.join("\n"), footer));
            }
            matcher::MatchResult::NoMatch => {
                lines.push(grumps_i18n::t(
                    locale,
                    "agent.todo.no_match",
                    &[("item", item.as_str())],
                ));
            }
        }
    }

    let mut messages = vec![OutboundMessage {
        text: lines.join("\n\n"),
        reply_to: Some(msg_id.to_string()),
        todo_id: None,
        markdown: false,
    }];
    messages.extend(extra_cards);
    Ok(HandlerResult::many(messages))
}

async fn handle_complete_single(
    target: CompletionTarget,
    ws_db: &WorkspaceDb<'_>,
    member_id: &str,
    msg_id: &str,
    slug: &str,
    locale: grumps_i18n::Locale,
) -> worker::Result<HandlerResult> {
    match target {
        CompletionTarget::BySeqNum(seq) => {
            let seq_str = seq.to_string();
            match ws_db.get_todo_by_seq(seq).await? {
                Some(todo) => {
                    ws_db.complete_todo(&todo.id, member_id).await?;
                    ws_db
                        .log_activity(member_id, "todo.completed", "todo", &todo.id, "chat")
                        .await?;
                    let mut text = grumps_i18n::t(
                        locale,
                        "agent.todo.completed",
                        &[("seq", &seq_str), ("title", &todo.title)],
                    );
                    let mut messages = Vec::new();
                    if let Some(ref rec) = todo.recurrence {
                        if !rec.is_empty() {
                            let tz = workspace_tz(ws_db).await;
                            if let Ok(next) = ws_db.create_next_recurrence(&todo, rec, tz).await {
                                text.push_str(&grumps_i18n::t(
                                    locale,
                                    "agent.card.next_occurrence",
                                    &[],
                                ));
                                messages.push(
                                    next_occurrence_card(ws_db, slug, &next, locale, tz).await,
                                );
                            }
                        }
                    }
                    messages.insert(
                        0,
                        OutboundMessage {
                            text,
                            reply_to: Some(msg_id.to_string()),
                            todo_id: None,
                            markdown: false,
                        },
                    );
                    Ok(HandlerResult::many(messages))
                }
                None => Ok(HandlerResult::one(
                    grumps_i18n::t(locale, "agent.todo.not_found", &[("seq", &seq_str)]),
                    Some(msg_id.to_string()),
                )),
            }
        }
        CompletionTarget::ByText(text) => {
            handle_complete_todos(vec![text], ws_db, member_id, msg_id, slug, locale).await
        }
    }
}

async fn handle_delete(
    seq: i64,
    ws_db: &WorkspaceDb<'_>,
    member_id: &str,
    locale: grumps_i18n::Locale,
) -> worker::Result<HandlerResult> {
    let seq_str = seq.to_string();
    match ws_db.get_todo_by_seq(seq).await? {
        Some(todo) => {
            ws_db.delete_todo(&todo.id).await?;
            ws_db
                .log_activity(member_id, "todo.deleted", "todo", &todo.id, "chat")
                .await?;
            Ok(HandlerResult::one(
                grumps_i18n::t(
                    locale,
                    "agent.todo.deleted",
                    &[("seq", &seq_str), ("title", &todo.title)],
                ),
                None,
            ))
        }
        None => Ok(HandlerResult::one(
            grumps_i18n::t(locale, "agent.todo.not_found", &[("seq", &seq_str)]),
            None,
        )),
    }
}

async fn handle_add_note(
    note: ParsedNote,
    ws_db: &WorkspaceDb<'_>,
    member_id: &str,
    locale: grumps_i18n::Locale,
    plan: &crate::billing::Plan,
) -> worker::Result<HandlerResult> {
    // Check note quota. Only `notes` is used → tz-irrelevant, pass UTC.
    let (_, _, note_count, _) = ws_db.get_status_counts("UTC").await?;
    if let Err(qe) = crate::billing::check_note_quota(plan, note_count) {
        return Ok(HandlerResult::one(qe.render(locale), None));
    }

    let title = note.title.as_deref().unwrap_or("");
    let note_id = ws_db
        .insert_note(title, &note.content, "chat", member_id)
        .await?;
    ws_db
        .log_activity(member_id, "note.created", "note", &note_id, "chat")
        .await?;
    let t = note
        .title
        .map(|t| format!(" \"{}\"", t))
        .unwrap_or_default();
    Ok(HandlerResult::one(
        grumps_i18n::t(locale, "agent.note.saved", &[("title", &t)]),
        None,
    ))
}

async fn handle_list_todos(
    filter: ListFilter,
    ws_db: &WorkspaceDb<'_>,
    member_id: &str,
    locale: grumps_i18n::Locale,
) -> worker::Result<HandlerResult> {
    let (actual_filter, actual_label): (String, String) = match &filter {
        ListFilter::Open => ("open".to_string(), "open".to_string()),
        ListFilter::All => ("all".to_string(), "all".to_string()),
        ListFilter::Mine => ("mine".to_string(), "mine".to_string()),
        ListFilter::Done => ("done".to_string(), "done".to_string()),
        ListFilter::Assignee(name) => (format!("assignee:{}", name), format!("@{}", name)),
        ListFilter::Tag(tag) => (format!("tag:{}", tag), format!("#{}", tag)),
    };

    let rows = ws_db
        .get_todos_filtered(&actual_filter, Some(member_id))
        .await?;
    let tz = workspace_tz(ws_db).await;
    let today = grumps_core::timeutil::today_in_tz(tz);

    let items: Vec<formatter::TodoListItem> = rows
        .into_iter()
        .map(
            |(_id, seq_num, title, status, assignee, priority, tags_json, deadline)| {
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                let priority = grumps_core::todo::Priority::from_int(priority);
                let priority_rank: u8 = match priority {
                    grumps_core::todo::Priority::High => 0,
                    grumps_core::todo::Priority::Normal => 1,
                    grumps_core::todo::Priority::Low => 2,
                };
                let (deadline_display, bucket, ordinal) =
                    deadline_display_info(locale, deadline.as_deref(), today);
                formatter::TodoListItem {
                    seq_num,
                    title,
                    done: status == "done",
                    assignee,
                    priority,
                    tags,
                    deadline_display,
                    sort_key: (bucket, ordinal, priority_rank, seq_num),
                }
            },
        )
        .collect();

    // Localized, pluralized count phrase — pre-rendered here (worker side)
    // since the formatter crate deliberately has no `grumps-i18n` dependency.
    let n = items.len() as i64;
    let header = match &filter {
        ListFilter::Open => grumps_i18n::t_plural(locale, "list.count.open", n, &[]),
        ListFilter::All => grumps_i18n::t_plural(locale, "list.count.all", n, &[]),
        ListFilter::Done => grumps_i18n::t_plural(locale, "list.count.done", n, &[]),
        ListFilter::Mine => grumps_i18n::t_plural(locale, "list.count.mine", n, &[]),
        ListFilter::Assignee(name) => {
            grumps_i18n::t_plural(locale, "list.count.assignee", n, &[("handle", name)])
        }
        ListFilter::Tag(tag) => {
            grumps_i18n::t_plural(locale, "list.count.tag", n, &[("handle", tag)])
        }
    };

    Ok(HandlerResult::one(
        formatter::todo_list(&items, &actual_label, &header, locale.code()),
        None,
    ))
}

async fn handle_list_notes(
    ws_db: &WorkspaceDb<'_>,
    locale: grumps_i18n::Locale,
) -> worker::Result<HandlerResult> {
    let notes = ws_db.get_notes().await?;
    Ok(HandlerResult::one(
        formatter::note_list(&notes, locale.code()),
        None,
    ))
}

async fn handle_search_notes(
    query: &str,
    ws_db: &WorkspaceDb<'_>,
    locale: grumps_i18n::Locale,
) -> worker::Result<HandlerResult> {
    let notes = ws_db.search_notes(query).await?;
    if notes.is_empty() {
        Ok(HandlerResult::one(
            grumps_i18n::t(locale, "agent.notes.search_empty", &[("query", query)]),
            None,
        ))
    } else {
        Ok(HandlerResult::one(
            formatter::note_list(&notes, locale.code()),
            None,
        ))
    }
}

async fn handle_status(
    ws_db: &WorkspaceDb<'_>,
    slug: &str,
    locale: grumps_i18n::Locale,
) -> worker::Result<HandlerResult> {
    // "Done this week" follows the workspace calendar.
    let tz = ws_db
        .get_setting("timezone")
        .await
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "UTC".to_string());
    let (open, done_week, notes, files) = ws_db.get_status_counts(&tz).await?;
    Ok(HandlerResult::one(
        formatter::status_summary(open, done_week, notes, files, slug, locale.code()),
        None,
    ))
}

async fn handle_quoted_todo(
    quoted_text: Option<&str>,
    msg_id: &str,
    ws_db: &WorkspaceDb<'_>,
    member_id: &str,
    slug: &str,
    locale: grumps_i18n::Locale,
    plan: &crate::billing::Plan,
) -> worker::Result<HandlerResult> {
    let content = quoted_text.unwrap_or("");
    if content.is_empty() {
        return Ok(HandlerResult::one(
            grumps_i18n::t(locale, "agent.quoted.no_todo", &[]),
            None,
        ));
    }
    let parsed = entity::extract_todo_from_line(content);
    handle_add_todos(vec![parsed], msg_id, ws_db, member_id, slug, locale, plan).await
}

async fn handle_quoted_note(
    quoted_text: Option<&str>,
    ws_db: &WorkspaceDb<'_>,
    member_id: &str,
    locale: grumps_i18n::Locale,
    plan: &crate::billing::Plan,
) -> worker::Result<HandlerResult> {
    let content = quoted_text.unwrap_or("");
    if content.is_empty() {
        return Ok(HandlerResult::one(
            grumps_i18n::t(locale, "agent.quoted.no_note", &[]),
            None,
        ));
    }
    let note = ParsedNote {
        title: None,
        content: content.to_string(),
    };
    handle_add_note(note, ws_db, member_id, locale, plan).await
}

async fn handle_card_reply(
    action: TaskCardAction,
    quoted_msg_id: Option<&str>,
    msg_id: &str,
    ws_db: &WorkspaceDb<'_>,
    member_id: &str,
    slug: &str,
    locale: grumps_i18n::Locale,
) -> worker::Result<HandlerResult> {
    // Look up which todo this reply refers to
    let todo_id = if let Some(qid) = quoted_msg_id {
        ws_db.get_todo_for_bot_message(qid).await.unwrap_or(None)
    } else {
        None
    };

    let mut extra_cards: Vec<OutboundMessage> = Vec::new();

    let text = match action {
        TaskCardAction::Done => {
            if let Some(ref tid) = todo_id {
                let mut text = grumps_i18n::t(locale, "agent.card.done", &[]);
                ws_db.complete_todo(tid, member_id).await?;
                ws_db
                    .log_activity(member_id, "todo.completed", "todo", tid, "chat")
                    .await?;
                if let Ok(Some(todo)) = ws_db.get_todo_by_id(tid).await {
                    if let Some(ref rec) = todo.recurrence {
                        if !rec.is_empty() {
                            let tz = workspace_tz(ws_db).await;
                            if let Ok(next) = ws_db.create_next_recurrence(&todo, rec, tz).await {
                                text.push_str(&grumps_i18n::t(
                                    locale,
                                    "agent.card.next_occurrence",
                                    &[],
                                ));
                                extra_cards.push(
                                    next_occurrence_card(ws_db, slug, &next, locale, tz).await,
                                );
                            }
                        }
                    }
                }
                text
            } else {
                grumps_i18n::t(locale, "agent.card.no_target", &[])
            }
        }
        TaskCardAction::Delete => {
            if let Some(ref tid) = todo_id {
                ws_db.delete_todo(tid).await?;
                ws_db
                    .log_activity(member_id, "todo.deleted", "todo", tid, "chat")
                    .await?;
                grumps_i18n::t(locale, "agent.card.deleted", &[])
            } else {
                grumps_i18n::t(locale, "agent.card.no_target", &[])
            }
        }
        TaskCardAction::Snooze(time) => {
            if let Some(ref tid) = todo_id {
                let tz = workspace_tz(ws_db).await;
                let today = grumps_core::timeutil::today_in_tz(tz);
                match resolve_snooze_date(&time, today) {
                    Some(date) => {
                        ws_db.set_todo_deadline(tid, &date).await?;
                        ws_db
                            .log_activity(member_id, "todo.snoozed", "todo", tid, "chat")
                            .await?;
                        grumps_i18n::t(locale, "agent.card.snoozed", &[("time", &date)])
                    }
                    None => grumps_i18n::t(locale, "agent.card.snooze_unparsed", &[]),
                }
            } else {
                grumps_i18n::t(locale, "agent.card.no_target", &[])
            }
        }
        TaskCardAction::Edit(title) => {
            if let Some(ref tid) = todo_id {
                ws_db
                    .update_todo(tid, Some(&title), None, None, None, None)
                    .await?;
                ws_db
                    .log_activity(member_id, "todo.edited", "todo", tid, "chat")
                    .await?;
                grumps_i18n::t(locale, "agent.card.updated", &[("title", &title)])
            } else {
                grumps_i18n::t(locale, "agent.card.no_target", &[])
            }
        }
        TaskCardAction::Reassign(person) => {
            if let Some(ref tid) = todo_id {
                // Chat replies only carry a display name, no resolved member
                // id — mirrors handle_add_todos, which also stores the same
                // mention text in both assignee columns.
                ws_db
                    .update_todo(tid, None, None, None, Some(&person), Some(&person))
                    .await?;
                ws_db
                    .log_activity(member_id, "todo.reassigned", "todo", tid, "chat")
                    .await?;
                grumps_i18n::t(locale, "agent.card.reassigned", &[("person", &person)])
            } else {
                grumps_i18n::t(locale, "agent.card.no_target", &[])
            }
        }
        TaskCardAction::ChangePriority(p) => {
            if let Some(ref tid) = todo_id {
                ws_db
                    .update_todo(tid, None, None, Some(p.as_int()), None, None)
                    .await?;
                ws_db
                    .log_activity(member_id, "todo.priority_changed", "todo", tid, "chat")
                    .await?;
                grumps_i18n::t(
                    locale,
                    "agent.card.priority",
                    &[("emoji", p.emoji()), ("label", p.label())],
                )
            } else {
                grumps_i18n::t(locale, "agent.card.no_target", &[])
            }
        }
        TaskCardAction::AddTag(tag) => {
            if let Some(ref tid) = todo_id {
                ws_db.add_todo_tag(tid, &tag).await?;
                ws_db
                    .log_activity(member_id, "todo.tag_added", "todo", tid, "chat")
                    .await?;
                grumps_i18n::t(locale, "agent.card.tag_added", &[("tag", &tag)])
            } else {
                grumps_i18n::t(locale, "agent.card.no_target", &[])
            }
        }
        TaskCardAction::ChangeStatus(s) => {
            if let Some(ref tid) = todo_id {
                ws_db
                    .update_todo(tid, None, Some(&s), None, None, None)
                    .await?;
                ws_db
                    .log_activity(member_id, "todo.status_changed", "todo", tid, "chat")
                    .await?;
                grumps_i18n::t(locale, "agent.card.status", &[("status", &s)])
            } else {
                grumps_i18n::t(locale, "agent.card.no_target", &[])
            }
        }
    };

    let mut messages = vec![OutboundMessage {
        text,
        reply_to: Some(msg_id.to_string()),
        todo_id: None,
        markdown: false,
    }];
    messages.extend(extra_cards);
    Ok(HandlerResult::many(messages))
}

/// Build the task card for a freshly-spawned recurrence occurrence, so a
/// reply to it (done/snooze/edit/...) resolves via `todo_id`. Marked
/// recurring (`↻`) since it was spawned by one; subject to the same
/// degressive hint / once-a-day link rule as every other card (`card_chrome`).
async fn next_occurrence_card(
    ws_db: &WorkspaceDb<'_>,
    slug: &str,
    next: &crate::db::NextOccurrence,
    locale: grumps_i18n::Locale,
    tz: chrono_tz::Tz,
) -> OutboundMessage {
    let today = grumps_core::timeutil::today_in_tz(tz);
    let (deadline_display, _, _) = deadline_display_info(locale, next.deadline.as_deref(), today);
    let (show_hint, show_link) = card_chrome(ws_db, tz).await;
    let card = formatter::task_card(
        next.seq_num,
        &next.title,
        next.assigned_name.as_deref(),
        deadline_display.as_deref(),
        grumps_core::todo::Priority::from_int(next.priority),
        &next.tags,
        true,
        show_hint,
        if show_link { Some(slug) } else { None },
    );
    OutboundMessage {
        text: card,
        reply_to: None,
        todo_id: Some(next.id.clone()),
        markdown: false,
    }
}

/// Resolve the workspace's IANA timezone setting, falling back to UTC.
async fn workspace_tz(ws_db: &WorkspaceDb<'_>) -> chrono_tz::Tz {
    let name = ws_db
        .get_setting("timezone")
        .await
        .ok()
        .flatten()
        .unwrap_or_default();
    grumps_core::timeutil::tz_or_utc(&name)
}

/// Compute whether the *next* card sent should show the degressive reply
/// hint (workspace's first ~5 cards) and/or the once-a-day workspace link,
/// recording the side effects (increment the hint counter, stamp today's
/// civil date) so subsequent calls see the update. Call exactly once per
/// card actually sent, or once per batch for a multi-todo add / multi-card
/// agent run — see call sites in `handle_add_todos`, `next_occurrence_card`,
/// `route_via_agent`, and `scheduler_executor::execute_action`.
pub(crate) async fn card_chrome(ws_db: &WorkspaceDb<'_>, tz: chrono_tz::Tz) -> (bool, bool) {
    let hint_count: i64 = ws_db
        .get_setting("card_hint_shown")
        .await
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let show_hint = hint_count < 5;
    if show_hint {
        let _ = ws_db.increment_setting_atomic("card_hint_shown", 1).await;
    }

    let today = grumps_core::timeutil::today_in_tz(tz)
        .format("%Y-%m-%d")
        .to_string();
    let last_link_date = ws_db.get_setting("card_link_date").await.ok().flatten();
    let show_link = last_link_date.as_deref() != Some(today.as_str());
    if show_link {
        let _ = ws_db.set_setting("card_link_date", &today).await;
    }

    (show_hint, show_link)
}

/// Compute the localized deadline display string for a card/list item, plus
/// a sort bucket + date ordinal driving the list's grouping order (see
/// `formatter::TodoListItem::sort_key`): `0` = overdue, `1` = today,
/// `2` = dated (future, ordered by `date_ordinal` ascending), `3` = undated.
///
/// - No stored deadline → `(None, 3, 0)`.
/// - Unparseable stored value (legacy raw text, e.g. an unresolved relative
///   hint) → shown as-is, sorted as undated.
/// - Overdue → `⚠ {weekday_abbr} {day}`.
/// - Today / tomorrow → localized `date.today` / `date.tomorrow`.
/// - Later → `{weekday_abbr} {day}`.
pub(crate) fn deadline_display_info(
    locale: grumps_i18n::Locale,
    deadline: Option<&str>,
    today: chrono::NaiveDate,
) -> (Option<String>, u8, i64) {
    let raw = match deadline.filter(|s| !s.is_empty()) {
        Some(d) => d,
        None => return (None, 3, 0),
    };
    let date = match chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return (Some(raw.to_string()), 3, 0),
    };
    let short = format!(
        "{} {}",
        grumps_i18n::t(locale, weekday_key(date.weekday()), &[]),
        date.day()
    );
    let days = (date - today).num_days();
    if days < 0 {
        (Some(format!("⚠ {short}")), 0, 0)
    } else if days == 0 {
        (Some(grumps_i18n::t(locale, "date.today", &[])), 1, 0)
    } else if days == 1 {
        (
            Some(grumps_i18n::t(locale, "date.tomorrow", &[])),
            2,
            date.num_days_from_ce() as i64,
        )
    } else {
        (Some(short), 2, date.num_days_from_ce() as i64)
    }
}

fn weekday_key(w: chrono::Weekday) -> &'static str {
    match w {
        chrono::Weekday::Mon => "date.wd.mon",
        chrono::Weekday::Tue => "date.wd.tue",
        chrono::Weekday::Wed => "date.wd.wed",
        chrono::Weekday::Thu => "date.wd.thu",
        chrono::Weekday::Fri => "date.wd.fri",
        chrono::Weekday::Sat => "date.wd.sat",
        chrono::Weekday::Sun => "date.wd.sun",
    }
}

/// Resolve a "snooze"/"snooze <text>" reply into a civil date (`YYYY-MM-DD`).
/// Empty input (bare "snooze") means "push to tomorrow". Otherwise delegates
/// to the shared nlu resolver (`resolve_relative_date`), so an explicit
/// `YYYY-MM-DD`, "tomorrow"/"demain", or a weekday name ("friday", "vendredi")
/// all work here too — not just on todo creation. Anything else is
/// unparseable and returns `None` so the caller leaves the todo untouched.
fn resolve_snooze_date(time: &str, today: chrono::NaiveDate) -> Option<String> {
    let trimmed = time.trim();
    if trimmed.is_empty() {
        return Some(
            (today + chrono::Duration::days(1))
                .format("%Y-%m-%d")
                .to_string(),
        );
    }
    entity::resolve_relative_date(trimmed, today).map(|d| d.format("%Y-%m-%d").to_string())
}

/// Last-resort routing: hand unrecognized free text to the LLM agent, which
/// classifies intent, may run tools, and sends its reply directly via the sink
/// (hence the empty `HandlerResult` on success — avoids a double send).
async fn route_via_agent(
    env: &Env,
    ws_db: &WorkspaceDb<'_>,
    ws_slug: &str,
    member_id: &str,
    text: &str,
    ws_locale: &str,
) -> worker::Result<HandlerResult> {
    let sink = crate::agent_sink::WorkerMessagingSink {
        env,
        ws_slug: ws_slug.to_string(),
        ws_db,
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
        Ok(route_result) => {
            // The agent may have created a scheduled_action / reminder; recompute
            // the workspace DO alarm so it fires promptly (the agent layer can't
            // arm the Durable Object itself). Best-effort.
            let _ = crate::routes::scheduled::reschedule_do(env, ws_slug).await;
            // The agent's own reply (already sent via the sink) is deliberately
            // terse about anything it created (see the RULES in prompt.rs) — the
            // real task card is rendered here, from what create_todo recorded,
            // exactly like the deterministic `TODO: x` parser path does.
            let locale = grumps_i18n::Locale::from_code(ws_locale);
            let tz = workspace_tz(ws_db).await;
            let today = grumps_core::timeutil::today_in_tz(tz);
            // Same batch rule as handle_add_todos: hint + link at most once,
            // on the first card — no separate summary message on this path,
            // so (unlike the multi-todo add path) the link isn't already
            // spoken for elsewhere.
            let (show_hint, show_link) = card_chrome(ws_db, tz).await;
            let grumps_agent::router::RouteResult {
                created_todos,
                completed_todos,
                updated_todos,
                deleted_todos,
                updated_notes,
                deleted_notes,
                cancelled_scheduled,
                updated_scheduled,
                ..
            } = route_result;
            let mut cards: Vec<OutboundMessage> = created_todos
                .into_iter()
                .enumerate()
                .map(|(idx, c)| {
                    let (deadline_display, _, _) =
                        deadline_display_info(locale, c.deadline.as_deref(), today);
                    OutboundMessage {
                        text: formatter::task_card(
                            c.seq_num,
                            &c.title,
                            c.assignee.as_deref(),
                            deadline_display.as_deref(),
                            c.priority,
                            &c.tags,
                            c.recurring,
                            show_hint && idx == 0,
                            if show_link && idx == 0 {
                                Some(ws_slug)
                            } else {
                                None
                            },
                        ),
                        reply_to: None,
                        todo_id: Some(c.id),
                        markdown: false,
                    }
                })
                .collect();

            // Same seam as created_todos above: the agent's own reply is
            // deliberately terse about anything it completed/updated/deleted
            // (see the RULES in prompt.rs) — the real, localized
            // acknowledgement is rendered here from what each tool recorded.
            cards.extend(mutation_ack_messages(
                locale,
                completed_todos,
                updated_todos,
                deleted_todos,
                updated_notes,
                deleted_notes,
                cancelled_scheduled,
                updated_scheduled,
            ));

            Ok(HandlerResult::many(cards))
        }
        Err(e) => {
            worker::console_log!("agent route failed for ws={ws_slug}: {e}");
            Ok(HandlerResult::one(
                grumps_i18n::t(
                    grumps_i18n::Locale::from_code(ws_locale),
                    "agent.error.technical",
                    &[],
                ),
                None,
            ))
        }
    }
}

/// Render the localized acknowledgement for every non-create mutation an
/// agent tool call performed during a run — the single-render-path
/// counterpart to the `created_todos` → task-card mapping above. Shared by
/// `route_via_agent` (chat path) and `scheduler_executor::execute_action`
/// (scheduled `agent_task`/`follow_up` path) so a "cancel the standup
/// reminder" instruction acks identically whether triggered live or by a
/// scheduled follow-up.
#[allow(clippy::too_many_arguments)]
pub(crate) fn mutation_ack_messages(
    locale: grumps_i18n::Locale,
    completed_todos: Vec<grumps_agent::tools::TodoAckCard>,
    updated_todos: Vec<grumps_agent::tools::TodoAckCard>,
    deleted_todos: Vec<grumps_agent::tools::TodoAckCard>,
    updated_notes: Vec<grumps_agent::tools::NoteAckCard>,
    deleted_notes: Vec<grumps_agent::tools::NoteAckCard>,
    cancelled_scheduled: Vec<grumps_agent::tools::ScheduledAckCard>,
    updated_scheduled: Vec<grumps_agent::tools::ScheduledAckCard>,
) -> Vec<OutboundMessage> {
    let mut out = Vec::new();
    for c in completed_todos {
        // Reuse the exact ack the deterministic `done #3` free-text command
        // path sends (`handle_mark_done`) — same key, same params.
        out.push(OutboundMessage {
            text: grumps_i18n::t(
                locale,
                "agent.todo.completed",
                &[("seq", &c.seq_num.to_string()), ("title", &c.title)],
            ),
            reply_to: None,
            todo_id: Some(c.id),
            markdown: false,
        });
    }
    for c in updated_todos {
        out.push(OutboundMessage {
            text: grumps_i18n::t(
                locale,
                "agent.tool.todo_updated",
                &[("seq", &c.seq_num.to_string()), ("title", &c.title)],
            ),
            reply_to: None,
            todo_id: Some(c.id),
            markdown: false,
        });
    }
    for c in deleted_todos {
        // Reuse the exact ack the deterministic `delete #3` free-text command
        // path sends (`handle_delete`) — same key, same params.
        out.push(OutboundMessage {
            text: grumps_i18n::t(
                locale,
                "agent.todo.deleted",
                &[("seq", &c.seq_num.to_string()), ("title", &c.title)],
            ),
            reply_to: None,
            todo_id: None, // the todo no longer exists — nothing to reply-anchor to
            markdown: false,
        });
    }
    for _ in updated_notes {
        out.push(OutboundMessage {
            text: grumps_i18n::t(locale, "agent.tool.note_updated", &[]),
            reply_to: None,
            todo_id: None,
            markdown: false,
        });
    }
    for _ in deleted_notes {
        out.push(OutboundMessage {
            text: grumps_i18n::t(locale, "agent.tool.note_deleted", &[]),
            reply_to: None,
            todo_id: None,
            markdown: false,
        });
    }
    for c in cancelled_scheduled {
        out.push(OutboundMessage {
            text: grumps_i18n::t(
                locale,
                "agent.tool.scheduled_cancelled",
                &[("title", &c.title)],
            ),
            reply_to: None,
            todo_id: None,
            markdown: false,
        });
    }
    for c in updated_scheduled {
        out.push(OutboundMessage {
            text: grumps_i18n::t(
                locale,
                "agent.tool.scheduled_updated",
                &[("title", &c.title)],
            ),
            reply_to: None,
            todo_id: None,
            markdown: false,
        });
    }
    out
}

/// Fast-path for silence/unsilence/explicit-memory commands addressed to
/// @grumps. Only fires when the message, once the leading mention token is
/// stripped, matches a known phrase exactly (or — for "remember that ..." —
/// starts with it): `entity::strip_fast_command_mention` requires the
/// mention to open the message and bails on `TODO:`/`DONE:`/`NOTE:` blocks,
/// so a bullet line that happens to *contain* "@grumps quiet" is parsed as
/// a todo instead of silencing the bot.
async fn try_fast_commands(
    env: &Env,
    ws_db: &WorkspaceDb<'_>,
    ws_slug: &str,
    member_id: &str,
    text: &str,
) -> worker::Result<Option<HandlerResult>> {
    let Some(rest) = entity::strip_fast_command_mention(text) else {
        return Ok(None);
    };
    let rest_lower = rest.to_lowercase();

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
    let silence_phrases = [
        "tais-toi",
        "quiet",
        "silence",
        "shh",
        "cállate",
        "cala-te",
        "cale-se",
        "ruhe",
        "zitto",
        "тише",
        "sus",
        "اسكت",
        "चुप",
        "安静",
        "静かに",
        "조용히",
        "diam",
    ];
    if silence_phrases.iter().any(|p| rest_lower == *p) {
        let silence_key = format!("proactive:{ws_slug}:silence_until");
        // Await the write — a dropped `execute()` future never runs, so the
        // silence window would never actually be recorded.
        if let Ok(p) = kv.put(&silence_key, "1") {
            let _ = p.expiration_ttl(86400).execute().await;
        }
        return Ok(Some(HandlerResult::one(
            grumps_i18n::t(locale, "agent.silence.confirm", &[]),
            None,
        )));
    }

    let unsilence_phrases = [
        "reviens",
        "unquiet",
        "come back",
        "vuelve",
        "volta",
        "zurück",
        "torna",
        "вернись",
        "geri dön",
        "عُد",
        "वापस आओ",
        "回来",
        "戻って",
        "돌아와",
        "kembali",
    ];
    if unsilence_phrases.iter().any(|p| rest_lower == *p) {
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
        "souviens-toi que ",
        "souviens-toi de ",
        "remember that ",
        "remember to ",
        "note that ",
    ];
    let memory_trigger = memory_phrases
        .iter()
        .find(|phrase| rest_lower.starts_with(**phrase));

    if let Some(phrase) = memory_trigger {
        // Extract the content from `rest` (preserving case) — `rest_lower`
        // and `rest` share the same byte offsets since `phrase` is ASCII.
        let content = rest[phrase.len()..].trim().to_string();
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
                Ok(_) => {
                    return Ok(Some(HandlerResult::one(
                        grumps_i18n::t(locale, "agent.memory.saved", &[]),
                        None,
                    )))
                }
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
