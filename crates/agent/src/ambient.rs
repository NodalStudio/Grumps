//! Unified ambient message classifier.
//! ONE Gemini Flash call per message that returns all 3 signals :
//!   - memorable (auto-memory extraction)
//!   - proactive (should bot chime in?)
//!   - feedback (quality signal : silence/forget/correction/etc.)
//!
//! Replaces the prior `auto_extract.rs` and `proactive.rs` modules.
//! See spec § 6.2 (auto-extract) + § 11 (proactive) + new : quality signals.

use crate::db::AgentDb;
use crate::router::MessagingSink;
use serde::{Deserialize, Serialize};
use worker::*;

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AmbientAnalysis {
    #[serde(default)]
    pub memorable: MemorableSignal,
    #[serde(default)]
    pub proactive: ProactiveSignal,
    #[serde(default)]
    pub feedback: FeedbackSignal,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct MemorableSignal {
    #[serde(default)]
    pub is_memorable: bool,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub related_member: Option<String>,
    #[serde(default)]
    pub confidence: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ProactiveSignal {
    #[serde(default)]
    pub should_intervene: bool,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct FeedbackSignal {
    /// null if no feedback signal detected
    #[serde(default)]
    pub signal: Option<String>, // "silence_request" | "forget_request" | "correction" | "praise" | "thanks" | "confusion"
    #[serde(default)]
    pub confidence: f32,
    #[serde(default)]
    pub reason: String,
    /// Optional reference to which recent bot action this responds to (described, not ID)
    #[serde(default)]
    pub target_recent_action: Option<String>,
}

/// Modes enabled for this workspace (read once before calling analyze).
pub struct AmbientModes {
    pub auto_memory: bool,
    pub proactive_mode: bool,
    pub feedback_detection: bool, // currently always true ; opt-out via setting `quality_feedback_disabled`
}

/// Recent bot action context (last ~30 min) — passed to classifier so it can attribute signals.
pub struct RecentBotAction {
    pub activity_id: String,
    pub activity_type: String, // 'bot.proactive_intervention' | 'bot.auto_memory_save' | etc
    pub age_seconds: i64,
    pub summary: String, // short human-readable
}

/// Run the unified classifier with telemetry recording.
pub async fn analyze_with_telemetry(
    env: &Env,
    db: &dyn crate::db::AgentDb,
    member_id: Option<&str>,
    text: &str,
    members: &[String],
    pinned_summary: &str,
    recent_actions: &[RecentBotAction],
    modes: &AmbientModes,
) -> AmbientAnalysis {
    let t0 = worker::Date::now().as_millis();
    let result = analyze(env, text, members, pinned_summary, recent_actions, modes).await;
    let latency_ms = (worker::Date::now().as_millis().saturating_sub(t0)) as u32;
    let record = crate::telemetry::LlmCallRecord::build(
        "ambient",
        "gemini",
        "gemini-2.5-flash",
        0,
        0,
        0,
        0,
        latency_ms,
        true,
        None,
        member_id.map(|s| s.to_string()),
        None,
        vec![],
    );
    crate::telemetry::record_call(db, record).await;
    result
}

/// Run the unified classifier. Returns the analysis ; on any error returns Default (all signals off).
pub async fn analyze(
    env: &Env,
    text: &str,
    members: &[String],
    pinned_summary: &str,
    recent_actions: &[RecentBotAction],
    modes: &AmbientModes,
) -> AmbientAnalysis {
    // Skip trivially short messages. Whitespace-word count works for
    // space-separated scripts; CJK (zh/ja/ko) has no spaces, so a substantive
    // sentence would count as one "word" — approximate via char count there so
    // those locales aren't silently excluded from ambient analysis.
    let token_estimate = text
        .split_whitespace()
        .count()
        .max(text.chars().count() / 4);
    if token_estimate < 3 {
        return Default::default();
    }
    if text.len() > 2000 {
        return Default::default();
    }
    if !modes.auto_memory && !modes.proactive_mode && !modes.feedback_detection {
        return Default::default();
    }

    let api_key = match env.secret("GEMINI_API_KEY") {
        Ok(k) => k.to_string(),
        Err(_) => return Default::default(),
    };
    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent?key={api_key}");

    let recent_actions_block = if recent_actions.is_empty() {
        "(no recent bot actions)".to_string()
    } else {
        recent_actions
            .iter()
            .map(|a| {
                format!(
                    "- {}s ago | {} | {}",
                    a.age_seconds, a.activity_type, a.summary
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let prompt = format!(
        r#"You analyze a chat message in a group conversation where an AI assistant 'Grumps' lives.
Group members: {members_list}
Pinned facts the bot knows: {pinned}
Recent bot actions in this group:
{recent_actions}

Message to analyze:
"{text}"

Return JSON with exactly these top-level fields:
{{
  "memorable": {{
    "is_memorable": bool,
    "kind": "fact"|"person"|"decision"|"preference"|"place"|"other",
    "key": "short slug or null",
    "value": "extracted content as a clean sentence",
    "related_member": "member name if about a person, or null",
    "confidence": 0.0-1.0
  }},
  "proactive": {{
    "should_intervene": bool,
    "reason": "why or why not"
  }},
  "feedback": {{
    "signal": "silence_request"|"forget_request"|"correction"|"praise"|"thanks"|"confusion"|null,
    "confidence": 0.0-1.0,
    "reason": "why",
    "target_recent_action": "describe which recent bot action this responds to, or null"
  }}
}}

Rules:
- memorable.is_memorable: only true for facts, decisions, preferences worth remembering. Skip small talk, questions, jokes, sensitive topics (health/money/conflict/romance), bot commands.
- proactive.should_intervene: only true if the bot has SPECIFIC, USEFUL info to add (from pinned facts). Skip small talk, ongoing emotional conversation, routine chatter. Default false.
- feedback.signal: detect user reactions to recent bot actions. Examples:
    "tais-toi", "ferme-la", "stop", "calme toi", "tu parles trop", "🤫", "ça suffit" → silence_request
    "oublie ça", "non c'est faux", "supprime ça" referring to a recent save → forget_request or correction
    "non plutôt X", "pas Y mais Z" within minutes of bot action → correction
    "merci", "👍", "parfait" after bot action → praise/thanks
    "j'ai pas compris", "?" after bot reply → confusion
  Be conservative — set signal=null if not clearly directed at the bot. Reject if the message addresses another member, not Grumps.

If the prompt explicitly mentions @grumps, that's still potentially feedback — analyze tone."#,
        members_list = if members.is_empty() {
            "(none listed)".to_string()
        } else {
            members.join(", ")
        },
        pinned = if pinned_summary.is_empty() {
            "(none)"
        } else {
            pinned_summary
        },
        recent_actions = recent_actions_block,
        text = text,
    );

    let req_body = serde_json::json!({
        "contents": [{ "role": "user", "parts": [{ "text": prompt }] }],
        "generationConfig": {
            "temperature": 0.1,
            "responseMimeType": "application/json",
            "maxOutputTokens": 512,
        }
    });
    let headers = Headers::new();
    headers.set("content-type", "application/json").ok();
    let req = match Request::new_with_init(
        &url,
        RequestInit::new()
            .with_method(Method::Post)
            .with_headers(headers)
            .with_body(Some(req_body.to_string().into())),
    ) {
        Ok(r) => r,
        Err(_) => return Default::default(),
    };

    let mut resp = match Fetch::Request(req).send().await {
        Ok(r) => r,
        Err(_) => return Default::default(),
    };
    if resp.status_code() >= 400 {
        return Default::default();
    }

    let parsed: serde_json::Value = match resp.json().await {
        Ok(p) => p,
        Err(_) => return Default::default(),
    };
    let text_resp = parsed
        .pointer("/candidates/0/content/parts/0/text")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    serde_json::from_str(text_resp.trim()).unwrap_or_else(|e| {
        console_log!("ambient classifier parse failed: {e}, raw: {text_resp}");
        Default::default()
    })
}

/// Apply the analysis : save memory if memorable, run proactive Sonnet filter if flagged,
/// log quality signal if feedback present.
pub async fn apply_analysis<'a>(
    env: &'a Env,
    db: &'a dyn AgentDb,
    sink: &'a dyn MessagingSink,
    workspace_slug: &str,
    ws_locale: &str,
    member_id: &str,
    raw_text: &str,
    analysis: &AmbientAnalysis,
    recent_actions: &[RecentBotAction],
) -> Result<()> {
    // 1. Save memory if memorable
    if analysis.memorable.is_memorable && analysis.memorable.confidence >= 0.5 {
        let kind = match analysis.memorable.kind.as_str() {
            "person" => grumps_memory::MemoryKind::Person,
            "decision" => grumps_memory::MemoryKind::Decision,
            "preference" => grumps_memory::MemoryKind::Preference,
            "place" => grumps_memory::MemoryKind::Place,
            "fact" => grumps_memory::MemoryKind::Fact,
            _ => grumps_memory::MemoryKind::Other,
        };
        let entry = grumps_memory::NewMemoryEntry {
            key: analysis.memorable.key.clone(),
            value: analysis.memorable.value.clone(),
            kind,
            related_member: analysis.memorable.related_member.clone(),
            tags: vec![],
            source: grumps_memory::MemorySource::ChatAuto,
            confidence: Some(analysis.memorable.confidence as f64),
            pinned: Some(false),
            expires_at: None,
            created_by: Some(member_id.to_string()),
        };
        if let Ok(mem_id) = db.create_memory(&entry).await {
            db.log_bot_action(
                "bot.auto_memory_save",
                &format!("saved {}", entry.value),
                Some(&mem_id),
            )
            .await
            .ok();
        }
    }

    // 2. Proactive intervention : Sonnet filter then maybe send
    if analysis.proactive.should_intervene {
        // Cooldown + rate limit check (KV)
        let kv = env.kv("KV").ok();
        let cooldown_key = format!("proactive:{workspace_slug}:lastfire");
        // Rate-limit bucket resets on the workspace-local hour boundary.
        let tz =
            grumps_core::timeutil::tz_or_utc(&db.get_setting("timezone").await.unwrap_or_default());
        let hour_key = format!(
            "proactive:{workspace_slug}:{}",
            chrono::Utc::now().with_timezone(&tz).format("%Y-%m-%d-%H")
        );
        let mut should_proceed = true;
        if let Some(ref kv) = kv {
            if kv.get(&cooldown_key).text().await.ok().flatten().is_some() {
                should_proceed = false;
            } else {
                let count: u32 = kv
                    .get(&hour_key)
                    .text()
                    .await
                    .ok()
                    .flatten()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                let max = db
                    .get_int_setting("proactive_max_per_hour")
                    .await
                    .unwrap_or(3) as u32;
                if count >= max {
                    should_proceed = false;
                }
            }
        }
        if should_proceed {
            // Escalate to the tool-enabled agent in PROACTIVE mode. It may either
            // propose a concrete action (a mutating tool call, staged for the user
            // to confirm) or simply chime in with text — both are posted to the
            // group by run_oneshot. A staged action is parked in KV for the
            // confirmation handler to execute once a human says "oui".
            // The workspace locale is the canonical source (index DB
            // `workspaces_meta.locale`), passed in by the caller. The reactive
            // path uses the same value, so proactive proposals — their text and
            // confirm/cancel buttons — render in the group's language.
            let language = if ws_locale.is_empty() {
                "en".to_string()
            } else {
                ws_locale.to_string()
            };
            let ctx = crate::tools::ToolContext {
                env,
                workspace_slug,
                member_id,
                sink,
                db,
                language,
                timezone: tz.name().to_string(),
                autonomy: crate::tools::Autonomy::Proactive,
            };
            let instruction = format!(
                "A group member just wrote (you were NOT addressed — you are observing proactively):\n\"{raw_text}\"\n\nClassifier reason: {reason}\n\nIf a concrete workspace action is clearly warranted — e.g. someone reports finishing a task, so the matching todo should be completed — use the appropriate tool. If instead you have a brief, specific, useful thing to say, say it. Otherwise reply with empty text and do nothing.",
                reason = analysis.proactive.reason
            );
            if let Ok(result) = crate::loop_::run_oneshot(&ctx, &instruction).await {
                let said_something = result
                    .final_text
                    .as_deref()
                    .map(|t| !t.trim().is_empty())
                    .unwrap_or(false);
                let intervened = result.staged_action.is_some() || said_something;
                if let Some(staged) = &result.staged_action {
                    // Park the proposed action for confirmation (group-scoped, short TTL).
                    if let Some(ref kv) = kv {
                        let pending = serde_json::json!({
                            "tool": staged.get("tool"),
                            "args": staged.get("args"),
                            "member_id": member_id,
                            "summary": result.final_text,
                            // Telegram message id of the proposal (with its
                            // inline buttons), so a button tap can edit/clear it.
                            "chat_message_id": result.staged_message_id,
                        });
                        if let Ok(p) = kv.put(
                            &format!("proactive:pending:{workspace_slug}"),
                            &pending.to_string(),
                        ) {
                            p.expiration_ttl(3600).execute().await.ok();
                        }
                    }
                    db.log_bot_action(
                        "bot.proactive_proposal",
                        result.final_text.as_deref().unwrap_or(""),
                        None,
                    )
                    .await
                    .ok();
                } else if said_something {
                    db.log_bot_action(
                        "bot.proactive_intervention",
                        result.final_text.as_deref().unwrap_or(""),
                        None,
                    )
                    .await
                    .ok();
                }
                if intervened {
                    if let Some(kv) = kv {
                        if let Ok(p) = kv.put(&cooldown_key, "1") {
                            p.expiration_ttl(60).execute().await.ok();
                        }
                        let cnt: u32 = kv
                            .get(&hour_key)
                            .text()
                            .await
                            .ok()
                            .flatten()
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                        if let Ok(p) = kv.put(&hour_key, &(cnt + 1).to_string()) {
                            p.expiration_ttl(7200).execute().await.ok();
                        }
                    }
                }
            }
        }
    }

    // 3. Quality signal : log it
    if let Some(signal) = &analysis.feedback.signal {
        if analysis.feedback.confidence >= 0.5 {
            // Try to attribute to a recent action
            let target = analysis
                .feedback
                .target_recent_action
                .as_ref()
                .and_then(|desc| {
                    recent_actions
                        .iter()
                        .find(|a| a.summary.contains(desc.as_str()) || desc.contains(&a.summary))
                });
            db.log_quality_signal(
                member_id,
                signal,
                target.map(|a| a.activity_id.as_str()),
                target.map(|a| a.activity_type.as_str()),
                raw_text,
                analysis.feedback.confidence as f64,
                &analysis.feedback.reason,
            )
            .await
            .ok();
        }
    }

    Ok(())
}
