// crates/worker/src/routes/stripe_webhook.rs
use crate::db;
use grumps_core::security::constant_time_eq;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use worker::*;

type HmacSha256 = Hmac<Sha256>;

/// Stripe recommends rejecting anything older than a few minutes to bound the
/// replay window even if a signed payload were ever intercepted.
/// https://docs.stripe.com/webhooks#verify-manually
const SIGNATURE_TOLERANCE_SECS: i64 = 300;

#[derive(Debug, PartialEq, Eq)]
pub enum StripeSignatureError {
    MissingHeader,
    MissingSecret,
    MalformedHeader,
    StaleTimestamp,
    NoMatchingSignature,
}

/// Verify a `Stripe-Signature` header against the raw request body, per the
/// `t=<timestamp>,v1=<hex hmac>[,v1=<hex hmac>...]` scheme Stripe uses.
///
/// Deliberately a pure function (no `worker::*` types) so it is natively
/// testable — no wasm target or Cloudflare bindings required to exercise the
/// parsing/HMAC/tolerance logic.
pub fn verify_stripe_signature(
    payload: &[u8],
    sig_header: Option<&str>,
    secret: Option<&str>,
    now_unix: i64,
) -> Result<(), StripeSignatureError> {
    let secret = secret
        .filter(|s| !s.is_empty())
        .ok_or(StripeSignatureError::MissingSecret)?;
    let header = sig_header
        .filter(|h| !h.is_empty())
        .ok_or(StripeSignatureError::MissingHeader)?;

    let mut timestamp: Option<i64> = None;
    let mut v1_sigs: Vec<&str> = Vec::new();
    for part in header.split(',') {
        let mut kv = part.splitn(2, '=');
        let key = kv.next().unwrap_or("").trim();
        let val = kv.next().unwrap_or("").trim();
        match key {
            "t" => timestamp = val.parse::<i64>().ok(),
            "v1" => {
                if !val.is_empty() {
                    v1_sigs.push(val);
                }
            }
            _ => {}
        }
    }
    let timestamp = timestamp.ok_or(StripeSignatureError::MalformedHeader)?;
    if v1_sigs.is_empty() {
        return Err(StripeSignatureError::MalformedHeader);
    }
    if (now_unix - timestamp).abs() > SIGNATURE_TOLERANCE_SECS {
        return Err(StripeSignatureError::StaleTimestamp);
    }

    // Signed payload is exactly `"{timestamp}.{raw body bytes}"` — build it as
    // bytes directly rather than through a `String` so a non-UTF8 body (should
    // never happen for Stripe's JSON, but let's not assume) can't get mangled
    // by a lossy conversion before hashing.
    let mut signed_payload = format!("{}.", timestamp).into_bytes();
    signed_payload.extend_from_slice(payload);

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| StripeSignatureError::MissingSecret)?;
    mac.update(&signed_payload);
    let expected = hex::encode(mac.finalize().into_bytes());

    let matched = v1_sigs
        .iter()
        .any(|sig| constant_time_eq(sig.as_bytes(), expected.as_bytes()));
    if matched {
        Ok(())
    } else {
        Err(StripeSignatureError::NoMatchingSignature)
    }
}

/// POST /webhook/stripe — handle Stripe events. The event only reaches the
/// handler logic below once `Stripe-Signature` has been verified against
/// `STRIPE_WEBHOOK_SECRET`; an unsigned or forged POST here can no longer
/// mutate `workspaces_meta.plan`.
pub async fn handle_stripe_webhook(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let body = req.bytes().await?;

    let sig_header = req.headers().get("Stripe-Signature")?;
    let secret = ctx
        .env
        .secret("STRIPE_WEBHOOK_SECRET")
        .ok()
        .map(|s| s.to_string());
    let now = chrono::Utc::now().timestamp();

    if let Err(e) = verify_stripe_signature(&body, sig_header.as_deref(), secret.as_deref(), now) {
        // Machine-to-machine endpoint (Stripe's retry logic only reads the
        // status code) — a short technical string here, same convention as
        // the other webhook routes' signature rejections.
        console_log!("Stripe webhook rejected: {:?}", e);
        return Response::error("Bad signature", 403);
    }

    let event: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|e| Error::RustError(format!("Invalid JSON: {}", e)))?;

    let event_type = event
        .pointer("/type")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    console_log!("Stripe event: {}", event_type);

    let index_db = db::get_index_db(&ctx.env)?;

    match event_type {
        "checkout.session.completed" => {
            // Customer subscribed — upgrade plan
            let slug = event
                .pointer("/data/object/metadata/workspace_slug")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let plan = event
                .pointer("/data/object/metadata/plan")
                .and_then(|v| v.as_str())
                .unwrap_or("pro");
            let _stripe_customer_id = event
                .pointer("/data/object/customer")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if !slug.is_empty() {
                index_db
                    .prepare("UPDATE workspaces_meta SET plan = ?1 WHERE slug = ?2")
                    .bind(&[plan.into(), slug.into()])?
                    .run()
                    .await?;
                console_log!("Upgraded workspace {} to plan {}", slug, plan);
            }
        }
        "customer.subscription.deleted" | "customer.subscription.updated" => {
            // Subscription cancelled or changed
            let status = event
                .pointer("/data/object/status")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let slug = event
                .pointer("/data/object/metadata/workspace_slug")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if status == "canceled" || status == "unpaid" {
                if !slug.is_empty() {
                    index_db
                        .prepare("UPDATE workspaces_meta SET plan = 'free' WHERE slug = ?1")
                        .bind(&[slug.into()])?
                        .run()
                        .await?;
                    console_log!("Downgraded workspace {} to free", slug);
                }
            }
        }
        _ => {
            console_log!("Unhandled Stripe event: {}", event_type);
        }
    }

    Response::ok("ok")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sign(secret: &str, timestamp: i64, payload: &[u8]) -> String {
        let mut signed_payload = format!("{}.", timestamp).into_bytes();
        signed_payload.extend_from_slice(payload);
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(&signed_payload);
        hex::encode(mac.finalize().into_bytes())
    }

    fn header(secret: &str, timestamp: i64, payload: &[u8]) -> String {
        format!("t={},v1={}", timestamp, sign(secret, timestamp, payload))
    }

    #[test]
    fn valid_signature_accepted() {
        let secret = "whsec_test_secret";
        let payload = br#"{"type":"checkout.session.completed"}"#;
        let now = 1_700_000_000i64;
        let h = header(secret, now, payload);
        assert!(verify_stripe_signature(payload, Some(&h), Some(secret), now).is_ok());
    }

    #[test]
    fn tampered_body_rejected() {
        let secret = "whsec_test_secret";
        let payload = br#"{"type":"checkout.session.completed"}"#;
        let now = 1_700_000_000i64;
        let h = header(secret, now, payload);
        let tampered = br#"{"type":"customer.subscription.deleted"}"#;
        assert_eq!(
            verify_stripe_signature(tampered, Some(&h), Some(secret), now),
            Err(StripeSignatureError::NoMatchingSignature)
        );
    }

    #[test]
    fn wrong_secret_rejected() {
        let payload = br#"{"type":"checkout.session.completed"}"#;
        let now = 1_700_000_000i64;
        let h = header("whsec_real", now, payload);
        assert_eq!(
            verify_stripe_signature(payload, Some(&h), Some("whsec_wrong"), now),
            Err(StripeSignatureError::NoMatchingSignature)
        );
    }

    #[test]
    fn stale_timestamp_rejected() {
        let secret = "whsec_test_secret";
        let payload = br#"{"type":"checkout.session.completed"}"#;
        let signed_at = 1_700_000_000i64;
        let h = header(secret, signed_at, payload);
        let now = signed_at + SIGNATURE_TOLERANCE_SECS + 1;
        assert_eq!(
            verify_stripe_signature(payload, Some(&h), Some(secret), now),
            Err(StripeSignatureError::StaleTimestamp)
        );
    }

    #[test]
    fn future_timestamp_beyond_tolerance_rejected() {
        // Clock skew in the other direction must also be rejected, not just staleness.
        let secret = "whsec_test_secret";
        let payload = br#"{"type":"checkout.session.completed"}"#;
        let signed_at = 1_700_000_000i64;
        let h = header(secret, signed_at, payload);
        let now = signed_at - SIGNATURE_TOLERANCE_SECS - 1;
        assert_eq!(
            verify_stripe_signature(payload, Some(&h), Some(secret), now),
            Err(StripeSignatureError::StaleTimestamp)
        );
    }

    #[test]
    fn missing_header_rejected() {
        let payload = br#"{"type":"checkout.session.completed"}"#;
        assert_eq!(
            verify_stripe_signature(payload, None, Some("whsec_test_secret"), 1_700_000_000),
            Err(StripeSignatureError::MissingHeader)
        );
    }

    #[test]
    fn missing_secret_rejected() {
        let secret = "whsec_test_secret";
        let payload = br#"{"type":"checkout.session.completed"}"#;
        let now = 1_700_000_000i64;
        let h = header(secret, now, payload);
        assert_eq!(
            verify_stripe_signature(payload, Some(&h), None, now),
            Err(StripeSignatureError::MissingSecret)
        );
    }

    #[test]
    fn malformed_header_rejected() {
        let payload = br#"{"type":"checkout.session.completed"}"#;
        assert_eq!(
            verify_stripe_signature(
                payload,
                Some("garbage"),
                Some("whsec_test_secret"),
                1_700_000_000
            ),
            Err(StripeSignatureError::MalformedHeader)
        );
    }

    #[test]
    fn matches_any_of_multiple_v1_signatures() {
        // Stripe emits multiple v1 signatures during secret rotation.
        let secret = "whsec_current";
        let payload = br#"{"type":"checkout.session.completed"}"#;
        let now = 1_700_000_000i64;
        let good = sign(secret, now, payload);
        let h = format!("t={},v1=deadbeef,v1={}", now, good);
        assert!(verify_stripe_signature(payload, Some(&h), Some(secret), now).is_ok());
    }
}
