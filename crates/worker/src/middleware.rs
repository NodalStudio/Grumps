use worker::*;
use serde::{Serialize, Deserialize};

const ALLOWED_ORIGINS: &[&str] = &["https://grumps.io", "https://www.grumps.io"];

/// Add CORS headers to a response.
pub fn add_cors(resp: &mut Response, origin: Option<&str>) -> Result<()> {
    let allowed = match origin {
        Some(o) if ALLOWED_ORIGINS.contains(&o) || o.starts_with("http://localhost") => o,
        _ => "http://localhost:8080",
    };
    let h = resp.headers_mut();
    h.set("Access-Control-Allow-Origin", allowed)?;
    h.set("Access-Control-Allow-Methods", "GET, POST, PUT, PATCH, DELETE, OPTIONS")?;
    h.set("Access-Control-Allow-Headers", "Content-Type, Authorization")?;
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
    pub sub: String,              // user_id
    pub phone: String,
    pub workspaces: Vec<String>,  // workspace slugs
    pub exp: usize,               // expiry timestamp
}

/// Verify JWT from Authorization: Bearer <token> header.
pub fn verify_jwt(req: &Request, secret: &str) -> std::result::Result<Claims, String> {
    let auth = req.headers().get("Authorization")
        .map_err(|_| "header error".to_string())?
        .ok_or("missing Authorization header")?;
    let token = auth.strip_prefix("Bearer ")
        .ok_or("invalid auth format — expected Bearer <token>")?;
    let key = jsonwebtoken::DecodingKey::from_secret(secret.as_bytes());
    let mut validation = jsonwebtoken::Validation::default();
    validation.validate_exp = true;
    let data = jsonwebtoken::decode::<Claims>(token, &key, &validation)
        .map_err(|e| format!("invalid token: {}", e))?;
    Ok(data.claims)
}

/// Check that the JWT claims include the requested workspace.
pub fn check_workspace_access(claims: &Claims, slug: &str) -> std::result::Result<(), String> {
    if claims.workspaces.contains(&slug.to_string()) {
        Ok(())
    } else {
        Err("not a member of this workspace".to_string())
    }
}

/// Create a signed JWT (7 day expiry).
pub fn create_jwt(user_id: &str, phone: &str, workspaces: Vec<String>, secret: &str) -> std::result::Result<String, String> {
    let exp = chrono::Utc::now().timestamp() as usize + 7 * 24 * 3600;
    let claims = Claims { sub: user_id.to_string(), phone: phone.to_string(), workspaces, exp };
    let key = jsonwebtoken::EncodingKey::from_secret(secret.as_bytes());
    jsonwebtoken::encode(&jsonwebtoken::Header::default(), &claims, &key)
        .map_err(|e| format!("jwt error: {}", e))
}

/// Helper: extract origin from request and wrap a response with CORS headers.
pub fn with_cors(req: &Request, mut resp: Response) -> Result<Response> {
    let origin = req.headers().get("Origin")?.unwrap_or_default();
    add_cors(&mut resp, Some(&origin))?;
    Ok(resp)
}

/// Returns true if the authenticated user's phone is in the SUPER_ADMIN_PHONES env var.
/// SUPER_ADMIN_PHONES is a comma-separated list of phone numbers (e.g. "+33612345678,+33612345679").
pub fn is_super_admin(env: &worker::Env, claims: &Claims) -> bool {
    let phones = env.var("SUPER_ADMIN_PHONES").map(|v| v.to_string()).unwrap_or_default();
    if phones.is_empty() { return false; }
    phones.split(',')
        .map(|p| p.trim())
        .any(|p| p == claims.phone)
}

/// Check if the current user has admin role in the given workspace.
/// Returns true if super_admin (overrides), or if their member.role == 'admin'.
pub async fn is_workspace_admin(env: &worker::Env, ws_db: &crate::db::WorkspaceDb<'_>, claims: &Claims) -> worker::Result<bool> {
    if is_super_admin(env, claims) { return Ok(true); }
    let role = ws_db.get_member_role(&claims.sub).await?.unwrap_or_default();
    Ok(role == "admin")
}
