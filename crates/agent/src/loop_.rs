//! The Sonnet agent loop : tool use, multi-turn, max 5 iterations.
//! See spec § 8.3.

use crate::llm::anthropic::{self, AnthropicRequest, ContentBlock, Message};
use crate::prompt::{self, MemberShort, PromptContext};
use crate::tools::{self, ToolContext};
use grumps_core::billing::Plan;
use grumps_i18n::{t, Locale};
use serde::Serialize;
use worker::*;

const MAX_TURNS: u32 = 5;
const MAX_TOKENS_PER_INVOCATION: u32 = 50_000;

#[derive(Debug, Clone, Serialize)]
pub struct LoopResult {
    pub final_text: Option<String>,
    pub turns: u32,
    pub total_tokens: u32,
}

pub async fn run_loop(ctx: &ToolContext<'_>, user_message: &str) -> Result<LoopResult> {
    // Check quota before doing anything
    let plan_str = ctx
        .db
        .get_setting("plan")
        .await
        .unwrap_or_else(|_| "free".into());
    let plan = Plan::from_str(&plan_str);
    let used = ctx
        .db
        .get_int_setting("agent_quota_used_month")
        .await
        .unwrap_or(0);
    let limit = i64::from(plan.agent_call_quota());
    if used >= limit {
        let locale = Locale::from_code(&ctx.language);
        let msg = t(
            locale,
            "agent.quota_exceeded",
            &[("limit", &limit.to_string()), ("plan", plan.as_str())],
        );
        ctx.sink.send(&msg).await.ok();
        return Ok(LoopResult {
            final_text: Some(msg),
            turns: 0,
            total_tokens: 0,
        });
    }

    let prompt_ctx = build_prompt_context(ctx).await?;
    let system_prompt = prompt::build_system_prompt(&prompt_ctx);
    let tools_json = tools::schemas::all_tools();

    // Load any existing session for this member
    let existing = ctx
        .db
        .get_active_agent_session(ctx.member_id)
        .await
        .ok()
        .flatten();
    let mut messages: Vec<Message> = existing
        .map(|s| s.messages.iter().map(session_msg_to_anthropic).collect())
        .unwrap_or_default();

    // Append the new user message
    messages.push(Message {
        role: "user".into(),
        content: serde_json::Value::String(user_message.to_string()),
    });

    let mut total_tokens = 0u32;
    let mut last_text: Option<String> = None;

    for turn in 1..=MAX_TURNS {
        let req = AnthropicRequest {
            system: Some(system_prompt.clone()),
            messages: messages.clone(),
            tools: tools_json.clone(),
            ..Default::default()
        };

        let resp = anthropic::call_with_telemetry(
            ctx.env,
            ctx.db,
            "agent_react",
            Some(ctx.member_id),
            None,
            &req,
        )
        .await?;
        ctx.db
            .increment_int_setting("agent_quota_used_month", 1)
            .await
            .ok();
        total_tokens =
            total_tokens.saturating_add(resp.usage.input_tokens + resp.usage.output_tokens);

        // Extract tool_use blocks and final text
        let mut tool_uses: Vec<(String, String, serde_json::Value)> = vec![];
        let mut text_pieces: Vec<String> = vec![];
        for block in &resp.content {
            match block {
                ContentBlock::Text { text } => text_pieces.push(text.clone()),
                ContentBlock::ToolUse { id, name, input } => {
                    tool_uses.push((id.clone(), name.clone(), input.clone()));
                }
            }
        }
        let assistant_text = text_pieces.join("\n");
        if !assistant_text.is_empty() {
            last_text = Some(assistant_text);
        }

        // Always append the assistant's full content as the next history entry
        let assistant_content_array: Vec<serde_json::Value> =
            resp.content.iter().map(content_block_to_json).collect();
        messages.push(Message {
            role: "assistant".into(),
            content: serde_json::Value::Array(assistant_content_array),
        });

        // If end_turn or no tool calls, we're done
        if resp.stop_reason == "end_turn" || tool_uses.is_empty() {
            persist_session(ctx, &messages).await.ok();
            if let Some(ref text) = last_text {
                ctx.sink.send(text).await?;
            }
            return Ok(LoopResult {
                final_text: last_text,
                turns: turn,
                total_tokens,
            });
        }

        // Dispatch each tool use, append tool_result messages
        let mut tool_results: Vec<serde_json::Value> = vec![];
        for (tu_id, tu_name, tu_input) in &tool_uses {
            let result = match tools::dispatch(ctx, tu_name, tu_input.clone()).await {
                Ok(v) => v,
                Err(e) => serde_json::json!({ "error": e.to_string() }),
            };
            tool_results.push(serde_json::json!({
                "type": "tool_result",
                "tool_use_id": tu_id,
                "content": serde_json::to_string(&result).unwrap_or_else(|_| "{}".into()),
            }));
        }
        messages.push(Message {
            role: "user".into(),
            content: serde_json::Value::Array(tool_results),
        });

        // Cumulative token cap
        if total_tokens >= MAX_TOKENS_PER_INVOCATION {
            console_log!("agent loop: token cap reached after {turn} turns");
            break;
        }
    }

    // Loop exited without end_turn (max turns or token cap)
    persist_session(ctx, &messages).await.ok();
    let locale = Locale::from_code(&ctx.language);
    let fallback = t(locale, "agent.fallback.unfinished", &[]);
    ctx.sink.send(&fallback).await.ok();
    Ok(LoopResult {
        final_text: Some(fallback),
        turns: MAX_TURNS,
        total_tokens,
    })
}

/// One-shot variant for autonomous runs (scheduled actions). No session persistence.
pub async fn run_oneshot(ctx: &ToolContext<'_>, instruction: &str) -> Result<LoopResult> {
    // Check quota
    let plan_str = ctx
        .db
        .get_setting("plan")
        .await
        .unwrap_or_else(|_| "free".into());
    let plan = Plan::from_str(&plan_str);
    let used = ctx
        .db
        .get_int_setting("agent_quota_used_month")
        .await
        .unwrap_or(0);
    let limit = i64::from(plan.agent_call_quota());
    if used >= limit {
        let locale = Locale::from_code(&ctx.language);
        let msg = t(
            locale,
            "agent.quota_exceeded",
            &[("limit", &limit.to_string()), ("plan", plan.as_str())],
        );
        ctx.sink.send(&msg).await.ok();
        return Ok(LoopResult {
            final_text: Some(msg),
            turns: 0,
            total_tokens: 0,
        });
    }

    let prompt_ctx = build_prompt_context(ctx).await?;
    let system_prompt = prompt::build_system_prompt(&prompt_ctx);
    let tools_json = tools::schemas::all_tools();

    let mut messages: Vec<Message> = vec![Message {
        role: "user".into(),
        content: serde_json::Value::String(instruction.to_string()),
    }];

    let mut total_tokens = 0u32;
    let mut last_text: Option<String> = None;

    for turn in 1..=MAX_TURNS {
        let req = AnthropicRequest {
            system: Some(system_prompt.clone()),
            messages: messages.clone(),
            tools: tools_json.clone(),
            ..Default::default()
        };
        let resp = anthropic::call_with_telemetry(
            ctx.env,
            ctx.db,
            "agent_oneshot",
            Some(ctx.member_id),
            None,
            &req,
        )
        .await?;
        ctx.db
            .increment_int_setting("agent_quota_used_month", 1)
            .await
            .ok();
        total_tokens =
            total_tokens.saturating_add(resp.usage.input_tokens + resp.usage.output_tokens);

        let mut tool_uses: Vec<(String, String, serde_json::Value)> = vec![];
        let mut text_pieces: Vec<String> = vec![];
        for block in &resp.content {
            match block {
                ContentBlock::Text { text } => text_pieces.push(text.clone()),
                ContentBlock::ToolUse { id, name, input } => {
                    tool_uses.push((id.clone(), name.clone(), input.clone()));
                }
            }
        }
        if !text_pieces.is_empty() {
            last_text = Some(text_pieces.join("\n"));
        }

        let assistant_content_array: Vec<serde_json::Value> =
            resp.content.iter().map(content_block_to_json).collect();
        messages.push(Message {
            role: "assistant".into(),
            content: serde_json::Value::Array(assistant_content_array),
        });

        if resp.stop_reason == "end_turn" || tool_uses.is_empty() {
            if let Some(ref text) = last_text {
                ctx.sink.send(text).await.ok();
            }
            return Ok(LoopResult {
                final_text: last_text,
                turns: turn,
                total_tokens,
            });
        }

        let mut tool_results: Vec<serde_json::Value> = vec![];
        for (tu_id, tu_name, tu_input) in &tool_uses {
            let result = match tools::dispatch(ctx, tu_name, tu_input.clone()).await {
                Ok(v) => v,
                Err(e) => serde_json::json!({ "error": e.to_string() }),
            };
            tool_results.push(serde_json::json!({
                "type": "tool_result",
                "tool_use_id": tu_id,
                "content": serde_json::to_string(&result).unwrap_or_else(|_| "{}".into()),
            }));
        }
        messages.push(Message {
            role: "user".into(),
            content: serde_json::Value::Array(tool_results),
        });

        if total_tokens >= MAX_TOKENS_PER_INVOCATION {
            break;
        }
    }

    Ok(LoopResult {
        final_text: last_text,
        turns: MAX_TURNS,
        total_tokens,
    })
}

async fn build_prompt_context(ctx: &ToolContext<'_>) -> Result<PromptContext> {
    let pinned = ctx.db.list_pinned_memory().await.unwrap_or_default();
    let db_members = ctx.db.list_active_members().await.unwrap_or_default();
    // Map db::MemberShort (display_name: Option<String>) → prompt::MemberShort (display_name: String)
    let members: Vec<MemberShort> = db_members
        .iter()
        .map(|m| MemberShort {
            display_name: m.display_name.clone().unwrap_or_else(|| m.id.clone()),
            role: m.role.clone(),
        })
        .collect();
    let now = chrono::Utc::now().to_rfc3339();

    // Quota : read from DB, derive from plan
    let plan_str = ctx
        .db
        .get_setting("plan")
        .await
        .unwrap_or_else(|_| "free".into());
    let plan = Plan::from_str(&plan_str);
    let agent_calls_used = ctx
        .db
        .get_int_setting("agent_quota_used_month")
        .await
        .unwrap_or(0) as u32;
    let agent_quota = plan.agent_call_quota();
    let agent_remaining = agent_quota.saturating_sub(agent_calls_used);

    let web_used = ctx
        .db
        .get_int_setting("web_search_quota_used_month")
        .await
        .unwrap_or(0) as u32;
    let web_quota = plan.web_search_quota();
    let web_remaining = web_quota.saturating_sub(web_used);

    Ok(PromptContext {
        workspace_name: ctx.workspace_slug.to_string(), // Plan C will fetch the real name
        platform: "telegram".to_string(),               // Plan C : look up from index DB
        member_count: members.len(),
        persona: "default".to_string(), // Plan C : read from settings
        language: ctx.language.clone(),
        pinned_memories: pinned,
        members,
        now_local: now,
        timezone: "UTC".to_string(), // Plan C : read from settings
        proactive_mode: false,       // Plan C : read from settings
        auto_memory: false,          // Plan C : read from settings
        agent_calls_remaining: agent_remaining,
        agent_calls_quota: agent_quota,
        web_search_remaining: web_remaining,
        web_search_quota: web_quota,
    })
}

fn session_msg_to_anthropic(m: &grumps_scheduler::SessionMessage) -> Message {
    Message {
        role: m.role.clone(),
        content: m.content.clone(),
    }
}

fn content_block_to_json(b: &ContentBlock) -> serde_json::Value {
    match b {
        ContentBlock::Text { text } => serde_json::json!({"type": "text", "text": text}),
        ContentBlock::ToolUse { id, name, input } => serde_json::json!({
            "type": "tool_use", "id": id, "name": name, "input": input
        }),
    }
}

async fn persist_session(ctx: &ToolContext<'_>, messages: &[Message]) -> Result<()> {
    // Convert Anthropic Messages to SessionMessage for storage, preserving all turns
    // including tool_use (array content) and tool_result turns verbatim.
    let sess_msgs: Vec<grumps_scheduler::SessionMessage> = messages
        .iter()
        .map(|m| grumps_scheduler::SessionMessage {
            role: m.role.clone(),
            content: m.content.clone(),
        })
        .collect();
    ctx.db
        .upsert_agent_session(ctx.member_id, &sess_msgs, None)
        .await?;
    Ok(())
}
