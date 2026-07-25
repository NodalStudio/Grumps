use worker::*;

/// KV-backed fixed-window per-IP counter. Returns Err if the caller has exceeded `limit`
/// requests in the current minute window. Best-effort (eventual consistency in KV) —
/// OK for v1 anti-spam, not a WAF.
pub async fn check_rate_limit(
    env: &Env,
    req: &Request,
    bucket: &str,
    limit: u32,
) -> std::result::Result<(), ()> {
    let ip = req
        .headers()
        .get("CF-Connecting-IP")
        .ok()
        .flatten()
        .unwrap_or_default();
    if ip.is_empty() {
        return Ok(());
    } // local dev / missing header → skip

    check_rate_limit_key(env, bucket, &ip, limit).await
}

/// Same fixed-window KV counter as [`check_rate_limit`], but keyed by an
/// arbitrary caller-supplied key instead of the request's IP. Use this for
/// buckets that need to bound abuse against a single *target* regardless of
/// how many source IPs it's spread across — e.g. per-phone OTP send/verify
/// limits, where an attacker rotating IPs must still be capped per phone
/// number.
pub async fn check_rate_limit_key(
    env: &Env,
    bucket: &str,
    key: &str,
    limit: u32,
) -> std::result::Result<(), ()> {
    if key.is_empty() {
        return Ok(());
    }

    let kv = match env.kv("KV") {
        Ok(k) => k,
        Err(_) => return Ok(()),
    };
    let window = chrono::Utc::now().timestamp() / 60;
    let full_key = format!("ratelimit:{}:{}:{}", bucket, key, window);

    let current: u32 = kv
        .get(&full_key)
        .text()
        .await
        .ok()
        .flatten()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    if current >= limit {
        return Err(());
    }

    // Await the write — a dropped `execute()` future never runs, so the
    // counter would never increment and the rate limit would never bind.
    if let Ok(p) = kv.put(&full_key, &(current + 1).to_string()) {
        let _ = p.expiration_ttl(120).execute().await;
    }
    Ok(())
}
