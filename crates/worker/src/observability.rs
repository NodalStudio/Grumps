use serde::Serialize;

/// Log a structured auth/workspace event for easy grep in `wrangler tail`.
/// Serialized as a single JSON line prefixed with `[event]`.
pub fn log_event<T: Serialize>(event: &str, fields: &T) {
    if let Ok(json) = serde_json::to_string(fields) {
        worker::console_log!("[event] {} {}", event, json);
    } else {
        worker::console_log!("[event] {}", event);
    }
}
