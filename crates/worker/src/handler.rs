// crates/worker/src/handler.rs
use grumps_messaging::adapter::OutboundMessage;
use grumps_messaging::formatter;
use grumps_nlu::parser::*;
use grumps_nlu::matcher;
use grumps_nlu::entity;
use grumps_nlu::llm::{NluIntent, NluResponse};
use crate::db::WorkspaceDb;
use crate::llm_client::LlmClient;

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
    parse_result: ParseResult,
    inbound_message_id: &str,
    inbound_quoted_message_id: Option<&str>,
    inbound_quoted_message_text: Option<&str>,
    sender_name: &str,
    ws_db: &WorkspaceDb<'_>,
    member_id: &str,
    workspace_slug: &str,
    llm_client: Option<&LlmClient>,
) -> worker::Result<HandlerResult> {
    match parse_result {
        ParseResult::AddTodos(todos) => handle_add_todos(todos, inbound_message_id, ws_db, member_id, workspace_slug).await,
        ParseResult::AddSingleTodo(todo) => {
            // If LLM client is available, classify free text instead of blindly creating a todo
            if let Some(llm) = llm_client {
                let original_text = &todo.title;
                let open_todos = ws_db.get_open_todos().await?;
                let todo_pairs: Vec<(i64, String)> = open_todos.iter().map(|(_, title, seq)| (*seq, title.clone())).collect();

                match llm.classify(original_text, sender_name, &todo_pairs).await {
                    Ok(nlu) => {
                        return handle_llm_result(nlu, todo, inbound_message_id, ws_db, member_id, workspace_slug).await;
                    }
                    Err(e) => {
                        worker::console_log!("LLM classify error: {}, falling back to AddSingleTodo", e);
                    }
                }
            }
            handle_add_todos(vec![todo], inbound_message_id, ws_db, member_id, workspace_slug).await
        }
        ParseResult::CompleteTodos(items) => handle_complete_todos(items, ws_db, member_id, inbound_message_id).await,
        ParseResult::CompleteSingle(target) => handle_complete_single(target, ws_db, member_id, inbound_message_id).await,
        ParseResult::DeleteTodo(seq) => handle_delete(seq, ws_db, member_id).await,
        ParseResult::AddNote(note) => handle_add_note(note, ws_db, member_id).await,
        ParseResult::ListTodos(filter) => handle_list_todos(filter, ws_db, member_id).await,
        ParseResult::ListNotes => handle_list_notes(ws_db).await,
        ParseResult::SearchNotes(query) => handle_search_notes(&query, ws_db).await,
        ParseResult::ListFiles => Ok(HandlerResult::one("File listing available on the web workspace.".into(), None)),
        ParseResult::Help => Ok(HandlerResult::one(formatter::help_text(), None)),
        ParseResult::WorkspaceLink => Ok(HandlerResult::one(format!("grumps.io/w/{}", workspace_slug), None)),
        ParseResult::Status => handle_status(ws_db, workspace_slug).await,
        ParseResult::QuotedTodo => handle_quoted_todo(inbound_quoted_message_text, inbound_message_id, ws_db, member_id, workspace_slug).await,
        ParseResult::QuotedNote => handle_quoted_note(inbound_quoted_message_text, ws_db, member_id).await,
        ParseResult::TaskCardReply(action) => handle_card_reply(action, inbound_quoted_message_id, inbound_message_id, ws_db, member_id).await,
        ParseResult::Ignore => Ok(HandlerResult::none()),
    }
}

async fn handle_add_todos(todos: Vec<ParsedTodo>, msg_id: &str, ws_db: &WorkspaceDb<'_>, member_id: &str, slug: &str) -> worker::Result<HandlerResult> {
    let mut messages = Vec::new();

    // Summary first
    messages.push(OutboundMessage {
        text: formatter::todos_added_summary(todos.len(), slug),
        reply_to: Some(msg_id.to_string()),
    });

    // Individual task card per todo
    for parsed in &todos {
        let tags_json = serde_json::to_string(&parsed.tags).unwrap_or_else(|_| "[]".into());
        let assignee = parsed.assignee_mention.as_deref().unwrap_or("");

        let (todo_id, seq) = ws_db.insert_todo(
            &parsed.title, parsed.priority.as_int(), &tags_json,
            assignee, assignee, member_id, "chat", msg_id,
        ).await?;

        ws_db.log_activity(member_id, "todo.created", "todo", &todo_id, "chat").await?;

        let card = formatter::task_card(
            seq, &parsed.title,
            parsed.assignee_mention.as_deref(),
            parsed.deadline_text.as_deref(),
            parsed.priority,
            &parsed.tags,
        );
        messages.push(OutboundMessage { text: card, reply_to: None });
    }

    Ok(HandlerResult::many(messages))
}

async fn handle_complete_todos(items: Vec<String>, ws_db: &WorkspaceDb<'_>, member_id: &str, msg_id: &str) -> worker::Result<HandlerResult> {
    let open_todos = ws_db.get_open_todos().await?;
    let mut lines = Vec::new();

    for item in &items {
        match matcher::match_done(item, &open_todos) {
            matcher::MatchResult::Exact(m) => {
                ws_db.complete_todo(&m.todo_id, member_id).await?;
                ws_db.log_activity(member_id, "todo.completed", "todo", &m.todo_id, "chat").await?;
                lines.push(format!("\u{2705} #{} \"{}\" \u{2014} done.", m.seq_num, m.title));
            }
            matcher::MatchResult::Fuzzy(candidates) => {
                let opts: Vec<String> = candidates.iter().enumerate()
                    .map(|(i, c)| format!("  {}. #{} \"{}\"", i + 1, c.seq_num, c.title)).collect();
                lines.push(format!("\u{1f50d} \"{}\" \u{2014} {} matches:\n{}\nReply with the number.",
                    item, candidates.len(), opts.join("\n")));
            }
            matcher::MatchResult::NoMatch => {
                lines.push(format!("\u{2753} \"{}\" \u{2014} no match.", item));
            }
        }
    }

    Ok(HandlerResult::one(lines.join("\n\n"), Some(msg_id.to_string())))
}

async fn handle_complete_single(target: CompletionTarget, ws_db: &WorkspaceDb<'_>, member_id: &str, msg_id: &str) -> worker::Result<HandlerResult> {
    match target {
        CompletionTarget::BySeqNum(seq) => {
            match ws_db.get_todo_by_seq(seq).await? {
                Some(todo) => {
                    ws_db.complete_todo(&todo.id, member_id).await?;
                    ws_db.log_activity(member_id, "todo.completed", "todo", &todo.id, "chat").await?;
                    Ok(HandlerResult::one(format!("\u{2705} #{} \"{}\" \u{2014} done.", seq, todo.title), Some(msg_id.to_string())))
                }
                None => Ok(HandlerResult::one(format!("\u{2753} No todo #{}.", seq), Some(msg_id.to_string()))),
            }
        }
        CompletionTarget::ByText(text) => handle_complete_todos(vec![text], ws_db, member_id, msg_id).await,
    }
}

async fn handle_delete(seq: i64, ws_db: &WorkspaceDb<'_>, member_id: &str) -> worker::Result<HandlerResult> {
    match ws_db.get_todo_by_seq(seq).await? {
        Some(todo) => {
            ws_db.delete_todo(&todo.id).await?;
            ws_db.log_activity(member_id, "todo.deleted", "todo", &todo.id, "chat").await?;
            Ok(HandlerResult::one(format!("\u{1f5d1}\u{fe0f} #{} \"{}\" \u{2014} deleted.", seq, todo.title), None))
        }
        None => Ok(HandlerResult::one(format!("\u{2753} No todo #{}.", seq), None)),
    }
}

async fn handle_add_note(note: ParsedNote, ws_db: &WorkspaceDb<'_>, member_id: &str) -> worker::Result<HandlerResult> {
    let title = note.title.as_deref().unwrap_or("");
    let note_id = ws_db.insert_note(title, &note.content, "chat", member_id).await?;
    ws_db.log_activity(member_id, "note.created", "note", &note_id, "chat").await?;
    let t = note.title.map(|t| format!(" \"{}\"", t)).unwrap_or_default();
    Ok(HandlerResult::one(format!("\u{1f4dd} Note{} saved.", t), None))
}

async fn handle_list_todos(filter: ListFilter, ws_db: &WorkspaceDb<'_>, member_id: &str) -> worker::Result<HandlerResult> {
    let (actual_filter, actual_label): (String, String) = match &filter {
        ListFilter::Open => ("open".to_string(), "open".to_string()),
        ListFilter::All => ("all".to_string(), "all".to_string()),
        ListFilter::Mine => ("mine".to_string(), "mine".to_string()),
        ListFilter::Done => ("done".to_string(), "done".to_string()),
        ListFilter::Assignee(name) => (format!("assignee:{}", name), format!("@{}", name)),
        ListFilter::Tag(tag) => (format!("tag:{}", tag), format!("#{}", tag)),
    };

    let todos = ws_db.get_todos_filtered(&actual_filter, Some(member_id)).await?;
    Ok(HandlerResult::one(formatter::todo_list(&todos, &actual_label), None))
}

async fn handle_list_notes(ws_db: &WorkspaceDb<'_>) -> worker::Result<HandlerResult> {
    let notes = ws_db.get_notes().await?;
    Ok(HandlerResult::one(formatter::note_list(&notes), None))
}

async fn handle_search_notes(query: &str, ws_db: &WorkspaceDb<'_>) -> worker::Result<HandlerResult> {
    let notes = ws_db.search_notes(query).await?;
    if notes.is_empty() {
        Ok(HandlerResult::one(format!("\u{1f50d} No notes matching \"{}\".", query), None))
    } else {
        Ok(HandlerResult::one(formatter::note_list(&notes), None))
    }
}

async fn handle_status(ws_db: &WorkspaceDb<'_>, slug: &str) -> worker::Result<HandlerResult> {
    let (open, done_week, notes, files) = ws_db.get_status_counts().await?;
    Ok(HandlerResult::one(formatter::status_summary(open, done_week, notes, files, slug), None))
}

async fn handle_quoted_todo(quoted_text: Option<&str>, msg_id: &str, ws_db: &WorkspaceDb<'_>, member_id: &str, slug: &str) -> worker::Result<HandlerResult> {
    let content = quoted_text.unwrap_or("");
    if content.is_empty() {
        return Ok(HandlerResult::one("\u{2753} No message content to turn into a todo.".into(), None));
    }
    let parsed = entity::extract_todo_from_line(content);
    handle_add_todos(vec![parsed], msg_id, ws_db, member_id, slug).await
}

async fn handle_quoted_note(quoted_text: Option<&str>, ws_db: &WorkspaceDb<'_>, member_id: &str) -> worker::Result<HandlerResult> {
    let content = quoted_text.unwrap_or("");
    if content.is_empty() {
        return Ok(HandlerResult::one("\u{2753} No message content to save as a note.".into(), None));
    }
    let note = ParsedNote { title: None, content: content.to_string() };
    handle_add_note(note, ws_db, member_id).await
}

async fn handle_card_reply(action: TaskCardAction, quoted_msg_id: Option<&str>, msg_id: &str, ws_db: &WorkspaceDb<'_>, member_id: &str) -> worker::Result<HandlerResult> {
    // Look up which todo this reply refers to
    let todo_id = if let Some(qid) = quoted_msg_id {
        ws_db.get_todo_for_bot_message(qid).await.unwrap_or(None)
    } else { None };

    let text = match action {
        TaskCardAction::Done => {
            if let Some(ref tid) = todo_id {
                ws_db.complete_todo(tid, member_id).await?;
                ws_db.log_activity(member_id, "todo.completed", "todo", tid, "chat").await?;
            }
            "Done.".into()
        }
        TaskCardAction::Delete => {
            if let Some(ref tid) = todo_id {
                ws_db.delete_todo(tid).await?;
                ws_db.log_activity(member_id, "todo.deleted", "todo", tid, "chat").await?;
            }
            "Deleted.".into()
        }
        TaskCardAction::Snooze(time) => format!("\u{23f0} Snoozed to {}.", time),
        TaskCardAction::Edit(title) => format!("\u{270f}\u{fe0f} Updated: \"{}\".", title),
        TaskCardAction::Reassign(person) => format!("\u{1f464} Reassigned to @{}.", person),
        TaskCardAction::ChangePriority(p) => format!("{} Priority: {}.", p.emoji(), p.label()),
        TaskCardAction::AddTag(tag) => format!("\u{1f3f7}\u{fe0f} #{}.", tag),
        TaskCardAction::ChangeStatus(s) => format!("\u{1f4cc} Status: {}.", s),
    };

    Ok(HandlerResult::one(text, Some(msg_id.to_string())))
}

async fn handle_llm_result(
    nlu: NluResponse,
    original_todo: ParsedTodo,
    msg_id: &str,
    ws_db: &WorkspaceDb<'_>,
    member_id: &str,
    slug: &str,
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
                todo.deadline_text = Some(deadline);
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
            handle_add_todos(vec![todo], msg_id, ws_db, member_id, slug).await
        }
        NluIntent::CompleteTodo => {
            if let Some(seq) = nlu.entities.target_id {
                handle_complete_single(CompletionTarget::BySeqNum(seq), ws_db, member_id, msg_id).await
            } else if let Some(title) = nlu.entities.title {
                handle_complete_single(CompletionTarget::ByText(title), ws_db, member_id, msg_id).await
            } else {
                // Fall back to treating original text as completion target
                handle_complete_todos(vec![original_todo.title], ws_db, member_id, msg_id).await
            }
        }
        NluIntent::AddNote => {
            let note = ParsedNote {
                title: nlu.entities.title.clone(),
                content: nlu.entities.title.unwrap_or(original_todo.title),
            };
            handle_add_note(note, ws_db, member_id).await
        }
        NluIntent::SetReminder => {
            let title = nlu.entities.title.unwrap_or("Reminder".into());
            let remind_at = nlu.entities.deadline.unwrap_or_else(|| "tomorrow 9:00".into());
            let target = nlu.entities.assignee.as_deref().unwrap_or(member_id);
            let recurrence = nlu.entities.recurrence.as_deref();

            let id = ws_db.insert_reminder(&title, &remind_at, recurrence, target, member_id).await?;
            ws_db.log_activity(member_id, "reminder.created", "reminder", &id, "chat").await?;

            let rec_text = recurrence.map(|r| format!(" ({})", r)).unwrap_or_default();
            let target_text = if target != member_id {
                format!(" for @{}", nlu.entities.assignee.as_deref().unwrap_or(""))
            } else {
                String::new()
            };

            Ok(HandlerResult::one(
                format!("\u{23f0} Reminder set{}: \"{}\"\n\u{1f4c5} {}{}", target_text, title, remind_at, rec_text),
                Some(msg_id.to_string()),
            ))
        }
        NluIntent::ListTodos => {
            handle_list_todos(ListFilter::Open, ws_db, member_id).await
        }
        NluIntent::ListNotes => {
            handle_list_notes(ws_db).await
        }
        NluIntent::SearchNotes => {
            let query = nlu.entities.search_query.unwrap_or(original_todo.title);
            handle_search_notes(&query, ws_db).await
        }
        NluIntent::DeleteTodo => {
            if let Some(seq) = nlu.entities.target_id {
                handle_delete(seq, ws_db, member_id).await
            } else {
                Ok(HandlerResult::one("Which todo do you want to delete? Use `delete #N`.".into(), Some(msg_id.to_string())))
            }
        }
        NluIntent::Summarize => {
            Ok(HandlerResult::one("Summarize is coming soon!".into(), Some(msg_id.to_string())))
        }
        NluIntent::Help => {
            Ok(HandlerResult::one(grumps_messaging::formatter::help_text(), None))
        }
        NluIntent::Status => {
            handle_status(ws_db, slug).await
        }
        NluIntent::Irrelevant => {
            Ok(HandlerResult::none())
        }
    }
}
