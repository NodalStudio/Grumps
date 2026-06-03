use serde::{Deserialize, Serialize};
use worker::*;

const ALLOWED_ORIGINS: &[&str] = &["https://grumps.app", "https://www.grumps.app"];

// Allow-list origin or echo localhost. Never emit "*" when Access-Control-Allow-Credentials is true.
pub fn add_cors(resp: &mut Response, origin: Option<&str>) -> Result<()> {
    let allowed = match origin {
        Some(o)
            if ALLOWED_ORIGINS.contains(&o)
                || o.starts_with("http://localhost")
                || o.starts_with("http://127.0.0.1") =>
        {
            o.to_string()
        }
        _ => "http://localhost:8080".to_string(),
    };
    let h = resp.headers_mut();
    h.set("Access-Control-Allow-Origin", &allowed)?;
    h.set("Access-Control-Allow-Credentials", "true")?;
    h.set("Vary", "Origin")?;
    h.set(
        "Access-Control-Allow-Methods",
        "GET, POST, PUT, PATCH, DELETE, OPTIONS",
    )?;
    h.set(
        "Access-Control-Allow-Headers",
        "Content-Type, Authorization, X-CSRF-Token",
    )?;
    h.set("Access-Control-Max-Age", "86400")?;
    Ok(())
}

/// Build a CORS preflight response (204 No Content).
pub fn preflight(req: &Request) -> Result<Response> {
    let origin = req.headers().get("Origin")?.unwrap_or_default();
    let mut resp = Response::empty()?;
    add_cors(&mut resp, Some(&origin))?;
    Ok(resp.with_status(204))
}

/// JWT claims structure.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,   // user_id
    pub phone: String, // legacy WA OTP flow — stays empty for cookie sessions
    pub workspaces: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sid: Option<String>, // session id for cookie-based sessions; None for legacy Bearer
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub csrf: Option<String>, // CSRF token for cookie-based sessions
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tg_user_id: Option<String>, // Telegram user id (Widget login) — used by super-admin gate
    pub exp: usize,
}

/// Verify JWT from Authorization: Bearer <token> header.
pub fn verify_jwt(req: &Request, secret: &str) -> std::result::Result<Claims, String> {
    let auth = req
        .headers()
        .get("Authorization")
        .map_err(|_| "header error".to_string())?
        .ok_or("missing Authorization header")?;
    let token = auth
        .strip_prefix("Bearer ")
        .ok_or("invalid auth format — expected Bearer <token>")?;
    let key = jsonwebtoken::DecodingKey::from_secret(secret.as_bytes());
    let mut validation = jsonwebtoken::Validation::default();
    validation.validate_exp = true;
    let data = jsonwebtoken::decode::<Claims>(token, &key, &validation)
        .map_err(|e| format!("invalid token: {}", e))?;
    Ok(data.claims)
}

/// Legacy JWT (no sid/csrf) — used by the WA OTP Bearer flow.
pub fn create_jwt(
    user_id: &str,
    phone: &str,
    workspaces: Vec<String>,
    secret: &str,
) -> std::result::Result<String, String> {
    let exp = chrono::Utc::now().timestamp() as usize + 7 * 24 * 3600;
    let claims = Claims {
        sub: user_id.into(),
        phone: phone.into(),
        workspaces,
        sid: None,
        csrf: None,
        tg_user_id: None,
        exp,
    };
    let key = jsonwebtoken::EncodingKey::from_secret(secret.as_bytes());
    jsonwebtoken::encode(&jsonwebtoken::Header::default(), &claims, &key)
        .map_err(|e| format!("jwt: {}", e))
}

/// New JWT for cookie-based sessions. `sid` is required for revocation checks;
/// `csrf` is required on mutating requests via the X-CSRF-Token header.
/// `tg_user_id` is the stable Telegram user id from the Widget login — used by
/// the super-admin gate (Telegram does not expose phone numbers via the Widget).
pub fn create_jwt_with_csrf(
    user_id: &str,
    workspaces: Vec<String>,
    sid: &str,
    csrf: &str,
    tg_user_id: Option<&str>,
    secret: &str,
) -> std::result::Result<String, String> {
    let exp = chrono::Utc::now().timestamp() as usize + 7 * 24 * 3600;
    let claims = Claims {
        sub: user_id.into(),
        phone: String::new(),
        workspaces,
        sid: Some(sid.into()),
        csrf: Some(csrf.into()),
        tg_user_id: tg_user_id.map(|s| s.to_string()),
        exp,
    };
    let key = jsonwebtoken::EncodingKey::from_secret(secret.as_bytes());
    jsonwebtoken::encode(&jsonwebtoken::Header::default(), &claims, &key)
        .map_err(|e| format!("jwt: {}", e))
}

/// Helper: extract origin from request and wrap a response with CORS headers.
pub fn with_cors(req: &Request, mut resp: Response) -> Result<Response> {
    let origin = req.headers().get("Origin")?.unwrap_or_default();
    add_cors(&mut resp, Some(&origin))?;
    Ok(resp)
}

/// Returns true if the authenticated user is a super-admin.
/// Primary path (cookie session via Telegram Widget): match `claims.tg_user_id`
/// against `SUPER_ADMIN_TELEGRAM_IDS` (comma-separated Telegram numeric ids).
/// Legacy path (WA OTP Bearer): match `claims.phone` against `SUPER_ADMIN_PHONES`.
/// Both lists are read from `env.secret()` first, with `env.var()` as a dev
/// fallback so .dev.vars without the secret declared still works locally.
pub fn is_super_admin(env: &worker::Env, claims: &Claims) -> bool {
    if let Some(tg_id) = claims.tg_user_id.as_deref() {
        if !tg_id.is_empty() {
            let ids = env
                .secret("SUPER_ADMIN_TELEGRAM_IDS")
                .map(|v| v.to_string())
                .or_else(|_| env.var("SUPER_ADMIN_TELEGRAM_IDS").map(|v| v.to_string()))
                .unwrap_or_default();
            if !ids.is_empty() && ids.split(',').map(str::trim).any(|s| s == tg_id) {
                return true;
            }
        }
    }
    if !claims.phone.is_empty() {
        let phones = env
            .secret("SUPER_ADMIN_PHONES")
            .map(|v| v.to_string())
            .or_else(|_| env.var("SUPER_ADMIN_PHONES").map(|v| v.to_string()))
            .unwrap_or_default();
        if !phones.is_empty() && phones.split(',').map(str::trim).any(|p| p == claims.phone) {
            return true;
        }
    }
    false
}

/// Check if a user has the `admin` role in a workspace, querying the index DB.
/// Reads the role from the index DB (no per-workspace DB connection needed).
pub async fn is_workspace_admin_by_slug(
    index_db: &worker::D1Database,
    user_id: &str,
    slug: &str,
) -> worker::Result<bool> {
    #[derive(serde::Deserialize)]
    struct Row {
        role: String,
    }
    let row = index_db
        .prepare("SELECT role FROM user_workspaces WHERE user_id = ?1 AND workspace_slug = ?2")
        .bind(&[user_id.into(), slug.into()])?
        .first::<Row>(None)
        .await?;
    Ok(row.map(|r| r.role == "admin").unwrap_or(false))
}

/// Returns a JSON error response with CORS headers already applied.
/// Use this in every handler's error branch so the browser sees the real status
/// instead of a generic "CORS error".
pub fn error_with_cors(req: &Request, status: u16, code: &str, detail: &str) -> Result<Response> {
    // Server-side faults get a structured error line tagged with the request's
    // correlation id; client errors (4xx) stay as the http.out warn line only.
    if status >= 500 {
        let rid = crate::observability::request_id(req);
        crate::observability::log_error(&rid, code, detail);
    }
    let body = serde_json::json!({ "error": code, "detail": detail });
    let mut resp = Response::from_json(&body)?.with_status(status);
    let origin = req.headers().get("Origin")?.unwrap_or_default();
    add_cors(&mut resp, Some(&origin))?;
    Ok(resp)
}

/// Extract a cookie value from the `Cookie` header. Returns None if header missing or key absent.
pub fn extract_cookie(req: &Request, name: &str) -> Option<String> {
    let cookies = req.headers().get("Cookie").ok().flatten()?;
    for part in cookies.split(';') {
        let kv = part.trim();
        if let Some(eq) = kv.find('=') {
            let (k, v) = kv.split_at(eq);
            if k == name {
                return Some(v[1..].to_string());
            }
        }
    }
    None
}

/// Build the Set-Cookie headers for session + CSRF. Domain=.grumps.app so the
/// cookie is shared between grumps.app and api.grumps.app subdomains. We also
/// emit explicit `Max-Age=0` cookies bound to the bare host (api.grumps.app)
/// so any leftover cookies from before the Domain rollout get cleared on the
/// same response — otherwise the browser keeps both and the host-bound one
/// shadows the apex-bound one.
pub fn set_auth_cookies(
    resp: &mut Response,
    jwt: &str,
    csrf: &str,
    environment: &str,
) -> Result<()> {
    let secure_attr = if environment == "production" {
        "Secure; "
    } else {
        ""
    };
    let max_age = 7 * 24 * 3600;
    let h = resp.headers_mut();
    if environment == "production" {
        // Clear stale host-only cookies from the api.grumps.app domain that may
        // have been issued before we set ENVIRONMENT=production.
        h.append(
            "Set-Cookie",
            "grumps_jwt=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax",
        )?;
        h.append(
            "Set-Cookie",
            "grumps_csrf=; Path=/; Max-Age=0; Secure; SameSite=Lax",
        )?;
    }
    let domain_attr = if environment == "production" {
        "Domain=.grumps.app; "
    } else {
        ""
    };
    h.append(
        "Set-Cookie",
        &format!(
            "grumps_jwt={}; {}Path=/; Max-Age={}; HttpOnly; {}SameSite=Lax",
            jwt, domain_attr, max_age, secure_attr
        ),
    )?;
    h.append(
        "Set-Cookie",
        &format!(
            "grumps_csrf={}; {}Path=/; Max-Age={}; {}SameSite=Lax",
            csrf, domain_attr, max_age, secure_attr
        ),
    )?;
    Ok(())
}

/// Expire both cookies — used by /auth/logout. Emits two clears each (with and
/// without Domain attr) so cookies from before the Domain rollout also expire.
pub fn clear_auth_cookies(resp: &mut Response, environment: &str) -> Result<()> {
    let h = resp.headers_mut();
    if environment == "production" {
        // Host-only (legacy api.grumps.app cookies)
        h.append(
            "Set-Cookie",
            "grumps_jwt=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax",
        )?;
        h.append(
            "Set-Cookie",
            "grumps_csrf=; Path=/; Max-Age=0; Secure; SameSite=Lax",
        )?;
        // Domain-scoped (current)
        h.append(
            "Set-Cookie",
            "grumps_jwt=; Domain=.grumps.app; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax",
        )?;
        h.append(
            "Set-Cookie",
            "grumps_csrf=; Domain=.grumps.app; Path=/; Max-Age=0; Secure; SameSite=Lax",
        )?;
    } else {
        h.append(
            "Set-Cookie",
            "grumps_jwt=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax",
        )?;
        h.append(
            "Set-Cookie",
            "grumps_csrf=; Path=/; Max-Age=0; SameSite=Lax",
        )?;
    }
    Ok(())
}

/// Check that a session is still active. Uses KV as a 60s cache over D1.
/// Returns Ok(()) if active, Err(message) if revoked or not found.
pub async fn check_session_active(env: &Env, sid: &str) -> std::result::Result<(), String> {
    let kv = env.kv("KV").map_err(|e| format!("kv: {:?}", e))?;
    let cache_key = format!("session:{}", sid);

    if kv
        .get(&cache_key)
        .text()
        .await
        .map_err(|e| format!("kv get: {:?}", e))?
        .is_some()
    {
        return Ok(());
    }

    let index_db = crate::db::get_index_db(env).map_err(|e| format!("db: {:?}", e))?;
    let active = crate::db::is_session_active(&index_db, sid)
        .await
        .map_err(|e| format!("db: {:?}", e))?;
    if !active {
        return Err("session revoked or missing".into());
    }

    let _ = kv
        .put(&cache_key, "valid")
        .map_err(|e| format!("kv put: {:?}", e))?
        .expiration_ttl(60)
        .execute()
        .await;

    Ok(())
}

/// Invalidate the KV cache entry after a revoke so propagation is immediate.
pub async fn invalidate_session_cache(env: &Env, sid: &str) -> std::result::Result<(), String> {
    let kv = env.kv("KV").map_err(|e| format!("kv: {:?}", e))?;
    let _ = kv.delete(&format!("session:{}", sid)).await;
    Ok(())
}

#[derive(Debug)]
pub enum AuthError {
    Unauthenticated,
    InvalidToken(String),
    SessionRevoked,
    CsrfMissing,
    CsrfMismatch,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::Unauthenticated => write!(f, "unauthenticated"),
            AuthError::InvalidToken(m) => write!(f, "invalid token: {}", m),
            AuthError::SessionRevoked => write!(f, "session revoked"),
            AuthError::CsrfMissing => write!(f, "csrf missing"),
            AuthError::CsrfMismatch => write!(f, "csrf mismatch"),
        }
    }
}

impl AuthError {
    pub fn status(&self) -> u16 {
        match self {
            AuthError::CsrfMismatch | AuthError::CsrfMissing => 403,
            _ => 401,
        }
    }
    pub fn code(&self) -> &'static str {
        match self {
            AuthError::Unauthenticated => "auth.unauthenticated",
            AuthError::InvalidToken(_) => "auth.invalid_token",
            AuthError::SessionRevoked => "auth.session_revoked",
            AuthError::CsrfMissing => "auth.csrf_missing",
            AuthError::CsrfMismatch => "auth.csrf_mismatch",
        }
    }
}

fn is_mutation_method(req: &Request) -> bool {
    matches!(
        req.method(),
        Method::Post | Method::Put | Method::Patch | Method::Delete
    )
}

/// Verify either a cookie session (preferred) or a Bearer token (legacy WA OTP).
/// On cookie path, also enforces CSRF on mutating methods and session revocation.
pub async fn verify_session(req: &Request, env: &Env) -> std::result::Result<Claims, AuthError> {
    let secret = env
        .secret("JWT_SECRET")
        .map_err(|_| AuthError::InvalidToken("no JWT_SECRET".into()))?
        .to_string();

    if let Some(jwt) = extract_cookie(req, "grumps_jwt") {
        let claims = decode_jwt_internal(&jwt, &secret).map_err(|e| AuthError::InvalidToken(e))?;

        // Revocation check.
        if let Some(sid) = &claims.sid {
            check_session_active(env, sid)
                .await
                .map_err(|_| AuthError::SessionRevoked)?;
        }

        // CSRF on mutations.
        if is_mutation_method(req) {
            let header = req.headers().get("X-CSRF-Token").ok().flatten();
            match (header, claims.csrf.clone()) {
                (Some(h), Some(c)) if h == c => {}
                (None, _) => return Err(AuthError::CsrfMissing),
                _ => return Err(AuthError::CsrfMismatch),
            }
        }

        return Ok(claims);
    }

    // Legacy Bearer path — keeps WA OTP working. If neither a cookie nor an
    // Authorization header is present, treat it as "no credentials" rather
    // than "bad token" so clients can distinguish the two.
    let has_auth_header = req.headers().get("Authorization").ok().flatten().is_some();
    if !has_auth_header {
        return Err(AuthError::Unauthenticated);
    }
    match verify_jwt(req, &secret) {
        Ok(c) => Ok(c),
        Err(e) => Err(AuthError::InvalidToken(e)),
    }
}

fn decode_jwt_internal(jwt: &str, secret: &str) -> std::result::Result<Claims, String> {
    let key = jsonwebtoken::DecodingKey::from_secret(secret.as_bytes());
    let mut validation = jsonwebtoken::Validation::default();
    validation.validate_exp = true;
    jsonwebtoken::decode::<Claims>(jwt, &key, &validation)
        .map(|d| d.claims)
        .map_err(|e| format!("{}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Regression guard: jsonwebtoken 10.x panics at the first `encode`/`decode`
    // if no CryptoProvider feature is selected (the cause of the auth 500 where
    // signing the session JWT crashed the worker). Exercising sign+verify here
    // means a missing/duplicate provider feature fails `cargo test` on the host,
    // long before it can reach a wasm deploy.
    #[test]
    fn jwt_round_trip_signs_and_verifies() {
        let secret = "test-secret";
        let token = create_jwt_with_csrf(
            "user-123",
            vec!["acme".into()],
            "sid-1",
            "csrf-tok",
            Some("987654321"),
            secret,
        )
        .expect("signing must not panic and must succeed");
        assert_eq!(token.split('.').count(), 3, "JWT must have three segments");

        let claims = decode_jwt_internal(&token, secret).expect("verify must succeed");
        assert_eq!(claims.sub, "user-123");
        assert_eq!(claims.sid.as_deref(), Some("sid-1"));
        assert_eq!(claims.csrf.as_deref(), Some("csrf-tok"));
        assert_eq!(claims.tg_user_id.as_deref(), Some("987654321"));
        assert_eq!(claims.workspaces, vec!["acme".to_string()]);
    }

    #[test]
    fn jwt_rejects_wrong_secret() {
        let token =
            create_jwt_with_csrf("u", vec![], "s", "c", None, "right-secret").expect("sign");
        assert!(decode_jwt_internal(&token, "wrong-secret").is_err());
    }
}
