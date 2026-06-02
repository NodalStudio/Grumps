use serde::Serialize;
use serde_json::json;

/// Log severity. Lowercased in output so `wrangler tail` / Workers Logs can be
/// grepped/filtered by `[info]` / `[warn]` / `[error]`.
#[derive(Clone, Copy)]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Info => "info",
            Level::Warn => "warn",
            Level::Error => "error",
        }
    }
}

/// Map an HTTP status to a log level: 5xx = error, 4xx = warn, else info.
pub fn level_for_status(status: u16) -> Level {
    if status >= 500 {
        Level::Error
    } else if status >= 400 {
        Level::Warn
    } else {
        Level::Info
    }
}

/// Derive a per-request correlation id. Prefers Cloudflare's `cf-ray` (unique
/// per request in prod). Falls back to "local" under `wrangler dev`, where
/// cf-ray is absent — webhook handlers add the platform message id to the log
/// fields so concurrent local replays stay distinguishable.
pub fn id_from_ray(ray: Option<&str>) -> String {
    match ray {
        Some(r) if !r.is_empty() => r.to_string(),
        _ => "local".to_string(),
    }
}

/// Extract the correlation id from a request's `cf-ray` header.
pub fn request_id(req: &worker::Request) -> String {
    id_from_ray(req.headers().get("cf-ray").ok().flatten().as_deref())
}

/// True when the `DEBUG_TRACE` env var is set to "1". Gates verbose per-branch
/// trace logs so production stays quiet; flip the var (Cloudflare dashboard or
/// `.dev.vars`) without redeploying logic.
pub fn trace_enabled(env: &worker::Env) -> bool {
    env.var("DEBUG_TRACE")
        .map(|v| v.to_string() == "1")
        .unwrap_or(false)
}

/// Core structured log line: `[level] event rid=<id> {json}`.
pub fn log(level: Level, rid: &str, event: &str, fields: &serde_json::Value) {
    worker::console_log!("[{}] {} rid={} {}", level.as_str(), event, rid, fields);
}

/// One line per inbound request, before routing.
pub fn log_request_in(rid: &str, method: &str, path: &str) {
    log(Level::Info, rid, "http.in", &json!({ "method": method, "path": path }));
}

/// One line per request after routing, with status + latency. Level follows
/// the status code so `[error]` flags 5xx.
pub fn log_request_out(rid: &str, status: u16, ms: u64) {
    log(level_for_status(status), rid, "http.out", &json!({ "status": status, "ms": ms }));
}

/// Structured error event with where-it-happened context.
pub fn log_error(rid: &str, location: &str, detail: &str) {
    log(Level::Error, rid, "error", &json!({ "where": location, "detail": detail }));
}

/// Log a structured event for easy grep in `wrangler tail`. Serialized as a
/// single JSON line prefixed with `[event]`. Retained for existing call sites.
pub fn log_event<T: Serialize>(event: &str, fields: &T) {
    if let Ok(json) = serde_json::to_string(fields) {
        worker::console_log!("[event] {} {}", event, json);
    } else {
        worker::console_log!("[event] {}", event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_from_ray_uses_ray_when_present() {
        assert_eq!(id_from_ray(Some("8abc123-DFW")), "8abc123-DFW");
    }

    #[test]
    fn id_from_ray_falls_back_when_missing_or_empty() {
        assert_eq!(id_from_ray(None), "local");
        assert_eq!(id_from_ray(Some("")), "local");
    }

    #[test]
    fn level_for_status_maps_ranges() {
        assert_eq!(level_for_status(200).as_str(), "info");
        assert_eq!(level_for_status(302).as_str(), "info");
        assert_eq!(level_for_status(404).as_str(), "warn");
        assert_eq!(level_for_status(403).as_str(), "warn");
        assert_eq!(level_for_status(500).as_str(), "error");
        assert_eq!(level_for_status(503).as_str(), "error");
    }
}
