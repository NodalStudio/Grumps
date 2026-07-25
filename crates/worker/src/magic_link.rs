// crates/worker/src/magic_link.rs
//
// Magic-link web login minted from `@grumps link` for platforms without a
// web login widget (WhatsApp / WAHA). Telegram keeps the Login Widget —
// `handler.rs`'s `WorkspaceLink` arm only calls into this module when the
// inbound platform is `"whatsapp"`.
//
// Security model (GitHub issue #24 — read this before touching the flow):
//   - Token: >=128 bits from a CSPRNG-backed helper (`middleware::random_b64_token`,
//     the same helper the widget-login CSRF token uses). The token itself
//     doubles as the KV key, so "look it up" is a direct point read, not a
//     linear/compare scan — no separate constant-time comparison is needed
//     (see `middleware::verify_session`'s CSRF check for where that pattern
//     *is* required, on a header the client controls).
//   - KV entry `magiclink:{token}` -> JSON `MagicLinkEntry`, 15-minute TTL
//     both via KV's native `expiration_ttl` and a belt-and-suspenders
//     `created_at` check in `classify_redemption` (defense in depth against
//     KV TTL edge cases / clock skew, and the thing that's actually
//     unit-testable without a live KV binding).
//   - Single-use: `routes::auth::handle_link_redeem` deletes the KV entry
//     immediately after the lookup and before any session is minted.
//   - DM-only delivery: `decide_delivery` is the pure decision function that
//     keeps "the signed link never appears in a group message" provable in
//     a unit test, independent of the WAHA HTTP call around it.
//   - Rate limit: 5 mints per member per hour via a dedicated KV counter
//     (`magiclink_rl:{member_id}`, TTL 3600s) — bounds DM spam from a
//     member spamming `@grumps link` in their group.

use crate::handler::HandlerResult;
use grumps_messaging::adapter::MessagingPlatform;
use serde::{Deserialize, Serialize};
use worker::Env;

/// KV TTL for a minted token, in seconds (15 minutes per the issue spec).
pub const TOKEN_TTL_SECS: i64 = 900;
const RATE_LIMIT_TTL_SECS: u64 = 3600;
const RATE_LIMIT_MAX: u32 = 5;
/// `GET /auth/link?token=...` — the worker's own domain, not the SPA's.
const AUTH_LINK_BASE: &str = "https://api.grumps.app/auth/link";

/// What the KV entry for a minted token carries. `platform_user_id` is the
/// sender's WAHA JID — the DM target for delivery, and later the identity
/// key `handle_link_redeem` resolves/creates a user for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MagicLinkEntry {
    pub member_id: String,
    pub platform_user_id: String,
    pub workspace_slug: String,
    pub created_at: i64,
}

/// How `@grumps link`'s reply is delivered, decided purely from whether the
/// inbound message came from a group. This is the one place the "signed
/// link never lands in a group" invariant lives — everything downstream
/// (handler.rs, the WAHA send) just follows what this returns, and the
/// invariant is checked by `delivery_never_puts_signed_link_in_group` below
/// without touching KV/WAHA at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatReplyKind {
    /// The chat reply itself is the signed link — safe only because the
    /// chat *is* the requester's own DM (no group ever gets this variant).
    SignedLink,
    /// The chat reply is the plain (unsigned) workspace URL plus a note
    /// that the real login link was sent by DM.
    PlainUrlWithDmNotice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkDelivery {
    pub chat_reply: ChatReplyKind,
    /// Whether a *separate* DM send (distinct from `chat_reply`) is needed
    /// to get the signed link to the requester. `false` when `chat_reply`
    /// already goes to their DM.
    pub send_separate_dm: bool,
}

/// Decide delivery for the `@grumps link` reply. `is_group` reflects the
/// inbound message's chat, i.e. `!inbound.is_direct_message`.
pub fn decide_delivery(is_group: bool) -> LinkDelivery {
    if is_group {
        LinkDelivery {
            chat_reply: ChatReplyKind::PlainUrlWithDmNotice,
            send_separate_dm: true,
        }
    } else {
        LinkDelivery {
            chat_reply: ChatReplyKind::SignedLink,
            send_separate_dm: false,
        }
    }
}

/// Outcome of classifying a redemption attempt. Deliberately has no
/// "not found" vs "expired" vs "malformed" variants — the issue requires a
/// uniform failure response, so the caller (`handle_link_redeem`) only ever
/// gets a yes/no answer and can't accidentally branch on *why* it failed.
pub fn classify_redemption(kv_value: Option<&str>, now: i64) -> Result<MagicLinkEntry, ()> {
    let raw = kv_value.ok_or(())?;
    let entry: MagicLinkEntry = serde_json::from_str(raw).map_err(|_| ())?;
    if now - entry.created_at > TOKEN_TTL_SECS {
        return Err(());
    }
    Ok(entry)
}

/// Build the signed magic-link URL for a freshly minted token.
fn link_url(token: &str) -> String {
    format!("{AUTH_LINK_BASE}?token={token}")
}

/// Mint a token, store its KV entry (15-min TTL), and return the signed URL.
/// `None` on any KV failure — the caller falls back to the pre-existing
/// plain-link reply rather than failing the whole `@grumps link` command.
async fn mint(
    env: &Env,
    member_id: &str,
    platform_user_id: &str,
    workspace_slug: &str,
) -> Option<String> {
    let kv = env.kv("KV").ok()?;
    let token = crate::middleware::random_b64_token(16); // 16 bytes = 128 bits
    let entry = MagicLinkEntry {
        member_id: member_id.to_string(),
        platform_user_id: platform_user_id.to_string(),
        workspace_slug: workspace_slug.to_string(),
        created_at: chrono::Utc::now().timestamp(),
    };
    let value = serde_json::to_string(&entry).ok()?;
    kv.put(&format!("magiclink:{token}"), &value)
        .ok()?
        .expiration_ttl(TOKEN_TTL_SECS as u64)
        .execute()
        .await
        .ok()?;
    Some(link_url(&token))
}

/// `true` if `member_id` has already minted 5 tokens in the last hour.
/// Deliberately a single counter with a resetting TTL (not a fixed-window
/// bucket like `rate_limit::check_rate_limit_key`) — simplest correct thing
/// per the issue's "5/hour" requirement without adding a second rate-limit
/// primitive to the codebase.
async fn rate_limited(env: &Env, member_id: &str) -> bool {
    let kv = match env.kv("KV") {
        Ok(k) => k,
        Err(_) => return false,
    };
    let key = format!("magiclink_rl:{member_id}");
    let count: u32 = kv
        .get(&key)
        .text()
        .await
        .ok()
        .flatten()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    if count >= RATE_LIMIT_MAX {
        return true;
    }
    if let Ok(p) = kv.put(&key, (count + 1).to_string()) {
        let _ = p.expiration_ttl(RATE_LIMIT_TTL_SECS).execute().await;
    }
    false
}

/// Send the signed link to `jid` via a standalone WAHA `sendText` call —
/// independent of the normal per-message send loop in `webhook_waha.rs`
/// (which always targets the *chat* JID, not the sender) precisely so a
/// group-originated request can't leak the link into the group. Best-effort:
/// logs a status code on failure but never the message body (it contains
/// the token).
async fn send_dm(env: &Env, jid: &str, text: String) {
    let (Ok(base_url), Ok(api_key), Ok(webhook_hmac)) = (
        env.secret("WAHA_URL").map(|v| v.to_string()),
        env.secret("WAHA_API_KEY").map(|v| v.to_string()),
        env.secret("WAHA_WEBHOOK_HMAC").map(|v| v.to_string()),
    ) else {
        worker::console_error!("magic link DM: missing WAHA secrets");
        return;
    };
    let session = env
        .var("WAHA_SESSION")
        .map(|v| v.to_string())
        .unwrap_or_else(|_| "default".into());
    let wa =
        grumps_messaging::waha::WahaAdapter::new(base_url, session, api_key.clone(), webhook_hmac);
    let msg = grumps_messaging::adapter::OutboundMessage::text(text);
    let Ok((url, body)) = wa.build_send_request(jid, &msg) else {
        worker::console_error!("magic link DM: failed to build send request");
        return;
    };

    let headers = worker::Headers::new();
    if headers.set("X-Api-Key", &api_key).is_err()
        || headers.set("Content-Type", "application/json").is_err()
    {
        return;
    }
    let mut init = worker::RequestInit::new();
    init.with_method(worker::Method::Post)
        .with_headers(headers)
        .with_body(Some(body.into()));
    let Ok(req) = worker::Request::new_with_init(&url, &init) else {
        return;
    };
    match worker::Fetch::Request(req).send().await {
        Ok(resp) => {
            let status = resp.status_code();
            if !(200..300).contains(&status) {
                worker::console_error!("magic link DM send failed: status={status}");
            }
        }
        Err(e) => worker::console_error!("magic link DM send error: {e:?}"),
    }
}

/// Handle `@grumps link` (`ParseResult::WorkspaceLink`). Telegram (and any
/// other platform with its own web login) is unchanged: just the plain
/// workspace URL. WhatsApp additionally mints a single-use magic-link token
/// and delivers it per `decide_delivery` — never as part of a group reply.
#[allow(clippy::too_many_arguments)]
pub async fn handle_workspace_link(
    env: Option<&Env>,
    member_id: &str,
    workspace_slug: &str,
    sender_id: &str,
    platform: &str,
    is_direct_message: bool,
    locale: grumps_i18n::Locale,
) -> worker::Result<HandlerResult> {
    let plain_line = format!("grumps.app/w/{workspace_slug}");

    if platform != "whatsapp" {
        return Ok(HandlerResult::one(plain_line, None));
    }
    let Some(env) = env else {
        return Ok(HandlerResult::one(plain_line, None));
    };

    if rate_limited(env, member_id).await {
        // Silent fallback: still useful (the plain URL), just no fresh
        // token. No i18n line for this — a hostile/looping caller doesn't
        // need an explanation, and a genuine user rarely hits 5/hour.
        return Ok(HandlerResult::one(plain_line, None));
    }

    let Some(link) = mint(env, member_id, sender_id, workspace_slug).await else {
        return Ok(HandlerResult::one(plain_line, None));
    };

    let delivery = decide_delivery(!is_direct_message);
    let dm_text = grumps_i18n::t(locale, "link.dm_message", &[("link", &link)]);

    match delivery.chat_reply {
        ChatReplyKind::SignedLink => {
            // Already in the requester's own DM — no separate send needed.
            debug_assert!(!delivery.send_separate_dm);
            Ok(HandlerResult::one(dm_text, None))
        }
        ChatReplyKind::PlainUrlWithDmNotice => {
            debug_assert!(delivery.send_separate_dm);
            send_dm(env, sender_id, dm_text).await;
            let notice = grumps_i18n::t(locale, "link.sent_by_dm", &[]);
            Ok(HandlerResult::one(format!("{plain_line}\n{notice}"), None))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decide_delivery_in_group_never_returns_signed_link() {
        let delivery = decide_delivery(true);
        assert_eq!(delivery.chat_reply, ChatReplyKind::PlainUrlWithDmNotice);
        assert!(
            delivery.send_separate_dm,
            "group requests must get the link via a separate DM send"
        );
    }

    #[test]
    fn decide_delivery_in_dm_replies_with_the_signed_link_directly() {
        let delivery = decide_delivery(false);
        assert_eq!(delivery.chat_reply, ChatReplyKind::SignedLink);
        assert!(
            !delivery.send_separate_dm,
            "an existing DM shouldn't trigger a second send"
        );
    }

    #[test]
    fn decide_delivery_is_exhaustive_over_bool() {
        // Guards against a future third state (e.g. "channel") being added
        // without updating this decision function — every possible
        // `is_group` value must map to a delivery that never puts the
        // signed link in `PlainUrlWithDmNotice`.
        for is_group in [true, false] {
            let delivery = decide_delivery(is_group);
            if delivery.chat_reply == ChatReplyKind::PlainUrlWithDmNotice {
                assert!(is_group, "only group chats get the plain+DM-notice reply");
            }
        }
    }

    fn sample_entry(created_at: i64) -> MagicLinkEntry {
        MagicLinkEntry {
            member_id: "member-1".into(),
            platform_user_id: "10000000001@c.us".into(),
            workspace_slug: "acme".into(),
            created_at,
        }
    }

    #[test]
    fn classify_redemption_accepts_a_fresh_entry() {
        let entry = sample_entry(1000);
        let raw = serde_json::to_string(&entry).unwrap();
        let outcome = classify_redemption(Some(&raw), 1000 + TOKEN_TTL_SECS - 1);
        assert_eq!(outcome, Ok(entry));
    }

    #[test]
    fn classify_redemption_rejects_a_missing_token() {
        assert_eq!(classify_redemption(None, 1000), Err(()));
    }

    #[test]
    fn classify_redemption_rejects_malformed_json() {
        assert_eq!(classify_redemption(Some("not json"), 1000), Err(()));
    }

    #[test]
    fn classify_redemption_rejects_an_expired_entry() {
        let entry = sample_entry(0);
        let raw = serde_json::to_string(&entry).unwrap();
        // One second past the 15-minute TTL.
        let outcome = classify_redemption(Some(&raw), TOKEN_TTL_SECS + 1);
        assert_eq!(outcome, Err(()));
    }

    #[test]
    fn classify_redemption_accepts_exactly_at_the_ttl_boundary() {
        let entry = sample_entry(0);
        let raw = serde_json::to_string(&entry).unwrap();
        assert_eq!(classify_redemption(Some(&raw), TOKEN_TTL_SECS), Ok(entry));
    }

    #[test]
    fn missing_and_expired_and_malformed_tokens_are_indistinguishable() {
        // The security requirement is that a caller can't tell *why*
        // redemption failed. All three failure modes must collapse to the
        // same `Err(())` — no variant to accidentally match on and expose.
        let expired_entry = sample_entry(0);
        let expired_raw = serde_json::to_string(&expired_entry).unwrap();
        let now = TOKEN_TTL_SECS + 1;

        let missing = classify_redemption(None, now);
        let malformed = classify_redemption(Some("garbage"), now);
        let expired = classify_redemption(Some(&expired_raw), now);

        assert_eq!(missing, Err(()));
        assert_eq!(malformed, Err(()));
        assert_eq!(expired, Err(()));
    }

    #[test]
    fn link_url_never_truncates_the_token() {
        let url = link_url("abcDEF123-_xyz");
        assert_eq!(url, format!("{AUTH_LINK_BASE}?token=abcDEF123-_xyz"));
    }
}
