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

    let kv = match env.kv("KV") {
        Ok(k) => k,
        Err(_) => return Ok(()),
    };
    let window = chrono::Utc::now().timestamp() / 60;
    let key = format!("ratelimit:{}:{}:{}", bucket, ip, window);

    let current: u32 = kv
        .get(&key)
        .text()
        .await
        .ok()
        .flatten()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    if current >= limit {
        return Err(());
    }

    let _ = kv
        .put(&key, &(current + 1).to_string())
        .and_then(|p| Ok(p.expiration_ttl(120)))
        .map(|b| b.execute());
    Ok(())
}
