// crates/worker/src/db/index.rs
//
// Index DB (native D1 binding). Tables: users, user_identities,
// workspaces_meta, user_workspaces, sessions.

use serde::{Deserialize, Serialize};
use worker::*;

// =============================================
// Index DB (native binding)
// =============================================

pub fn get_index_db(env: &Env) -> Result<D1Database> {
    env.d1("INDEX_DB")
}

#[derive(Deserialize, Debug, Clone)]
pub struct WorkspaceMetaRow {
    pub slug: String,
    pub d1_database_id: String,
    pub name: Option<String>,
    pub plan: String,
    pub locale: String,
}

pub async fn lookup_workspace_by_slug(
    index_db: &D1Database,
    slug: &str,
) -> Result<Option<WorkspaceMetaRow>> {
    index_db
        .prepare(
            "SELECT slug, d1_database_id, name, plan, locale FROM workspaces_meta WHERE slug = ?1",
        )
        .bind(&[slug.into()])?
        .first::<WorkspaceMetaRow>(None)
        .await
}

pub async fn lookup_workspace(
    index_db: &D1Database,
    platform: &str,
    channel_id: &str,
) -> Result<Option<WorkspaceMetaRow>> {
    index_db.prepare("SELECT slug, d1_database_id, name, plan, locale FROM workspaces_meta WHERE platform = ?1 AND platform_channel_id = ?2")
        .bind(&[platform.into(), channel_id.into()])?.first::<WorkspaceMetaRow>(None).await
}

/// Update the locale column on workspaces_meta for the given slug.
/// Caller is responsible for validating that `locale` is a supported code.
pub async fn update_workspace_locale(
    index_db: &D1Database,
    slug: &str,
    locale: &str,
) -> Result<()> {
    index_db
        .prepare("UPDATE workspaces_meta SET locale = ?1 WHERE slug = ?2")
        .bind(&[locale.into(), slug.into()])?
        .run()
        .await?;
    Ok(())
}

/// Returns `(platform, platform_channel_id)` for the workspace, or `None`
/// if the slug doesn't exist. Used by cross-cutting actions that need to
/// call the right platform adapter (e.g. re-applying setChatDescription).
pub async fn lookup_platform_channel(
    index_db: &D1Database,
    slug: &str,
) -> Result<Option<(String, String)>> {
    #[derive(Deserialize)]
    struct Row {
        platform: String,
        platform_channel_id: String,
    }
    let row = index_db
        .prepare("SELECT platform, platform_channel_id FROM workspaces_meta WHERE slug = ?1")
        .bind(&[slug.into()])?
        .first::<Row>(None)
        .await?;
    Ok(row.map(|r| (r.platform, r.platform_channel_id)))
}

/// Upsert a user in the Index DB and link them to a workspace.
pub async fn upsert_index_user(
    index_db: &D1Database,
    phone: &str,
    workspace_slug: &str,
    role: &str,
) -> Result<()> {
    let _ = upsert_identity_user(index_db, "whatsapp", phone, workspace_slug, role, None).await?;
    Ok(())
}

/// Find the Grumps user_id owning a (platform, platform_user_id) identity, if any.
pub async fn lookup_user_by_identity(
    index_db: &D1Database,
    platform: &str,
    platform_user_id: &str,
) -> Result<Option<String>> {
    #[derive(Deserialize)]
    struct Row {
        user_id: String,
    }
    let row = index_db
        .prepare(
            "SELECT user_id FROM user_identities WHERE platform = ?1 AND platform_user_id = ?2",
        )
        .bind(&[platform.into(), platform_user_id.into()])?
        .first::<Row>(None)
        .await?;
    Ok(row.map(|r| r.user_id))
}

/// Create (user + identity) if the identity doesn't exist, then link to a workspace.
/// Idempotent: safe to call on every message from a TG group member. Returns the user_id.
pub async fn upsert_identity_user(
    index_db: &D1Database,
    platform: &str,
    platform_user_id: &str,
    workspace_slug: &str,
    role: &str,
    display_name: Option<&str>,
) -> Result<String> {
    let user_id = match lookup_user_by_identity(index_db, platform, platform_user_id).await? {
        Some(uid) => uid,
        None => {
            let new_uid = uuid::Uuid::new_v4().to_string();
            index_db
                .prepare("INSERT INTO users (id, display_name) VALUES (?1, ?2)")
                .bind(&[
                    new_uid.clone().into(),
                    display_name.unwrap_or_default().into(),
                ])?
                .run()
                .await?;
            index_db
                .prepare("INSERT INTO user_identities (platform, platform_user_id, user_id) VALUES (?1, ?2, ?3)")
                .bind(&[platform.into(), platform_user_id.into(), new_uid.clone().into()])?
                .run().await?;
            new_uid
        }
    };

    index_db
        .prepare(
            "INSERT INTO user_workspaces (user_id, workspace_slug, role) VALUES (?1, ?2, ?3) \
                  ON CONFLICT(user_id, workspace_slug) DO NOTHING",
        )
        .bind(&[user_id.clone().into(), workspace_slug.into(), role.into()])?
        .run()
        .await?;

    Ok(user_id)
}

/// List identities of a user (for /api/me/identities / settings page).
/// Intended API, not yet wired to a route.
#[allow(dead_code)]
#[derive(Serialize)]
pub struct UserIdentity {
    pub platform: String,
    pub platform_user_id: String,
    pub verified_at: String,
}

pub async fn list_user_identities(
    index_db: &D1Database,
    user_id: &str,
) -> Result<Vec<UserIdentity>> {
    #[derive(Deserialize)]
    struct Row {
        platform: String,
        platform_user_id: String,
        verified_at: String,
    }
    let res = index_db
        .prepare("SELECT platform, platform_user_id, verified_at FROM user_identities WHERE user_id = ?1 ORDER BY verified_at")
        .bind(&[user_id.into()])?
        .all().await?;
    let rows: Vec<Row> = res.results()?;
    Ok(rows
        .into_iter()
        .map(|r| UserIdentity {
            platform: r.platform,
            platform_user_id: r.platform_user_id,
            verified_at: r.verified_at,
        })
        .collect())
}

/// Update user display_name and/or default_locale.
pub async fn update_user_profile(
    index_db: &D1Database,
    user_id: &str,
    display_name: Option<&str>,
    default_locale: Option<&str>,
) -> Result<()> {
    // SQLite's COALESCE lets us patch selectively in one statement.
    index_db
        .prepare("UPDATE users SET display_name = COALESCE(?2, display_name), default_locale = COALESCE(?3, default_locale) WHERE id = ?1")
        .bind(&[
            user_id.into(),
            display_name.map(|s| s.into()).unwrap_or(worker::wasm_bindgen::JsValue::NULL),
            default_locale.map(|s| s.into()).unwrap_or(worker::wasm_bindgen::JsValue::NULL),
        ])?
        .run().await?;
    Ok(())
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SessionRow {
    pub id: String,
    pub user_id: String,
    pub user_agent: Option<String>,
    pub device_label: Option<String>,
    pub country_hint: Option<String>,
    pub created_at: String,
    pub last_seen_at: String,
    pub revoked_at: Option<String>,
}

pub async fn create_session(
    index_db: &D1Database,
    session_id: &str,
    user_id: &str,
    user_agent: Option<&str>,
    device_label: Option<&str>,
    country_hint: Option<&str>,
) -> Result<()> {
    index_db
        .prepare("INSERT INTO sessions (id, user_id, user_agent, device_label, country_hint) VALUES (?1, ?2, ?3, ?4, ?5)")
        .bind(&[
            session_id.into(), user_id.into(),
            user_agent.unwrap_or("").into(),
            device_label.unwrap_or("").into(),
            country_hint.unwrap_or("").into(),
        ])?
        .run().await?;
    Ok(())
}

pub async fn is_session_active(index_db: &D1Database, session_id: &str) -> Result<bool> {
    #[derive(Deserialize)]
    struct Row {
        _ignored: Option<i64>,
    }
    let row = index_db
        .prepare("SELECT 1 as _ignored FROM sessions WHERE id = ?1 AND revoked_at IS NULL")
        .bind(&[session_id.into()])?
        .first::<Row>(None)
        .await?;
    Ok(row.is_some())
}

pub async fn list_active_sessions(index_db: &D1Database, user_id: &str) -> Result<Vec<SessionRow>> {
    let res = index_db
        .prepare("SELECT id, user_id, user_agent, device_label, country_hint, created_at, last_seen_at, revoked_at \
                  FROM sessions WHERE user_id = ?1 AND revoked_at IS NULL ORDER BY last_seen_at DESC")
        .bind(&[user_id.into()])?
        .all().await?;
    Ok(res.results()?)
}

pub async fn revoke_session(
    index_db: &D1Database,
    session_id: &str,
    user_id: &str,
) -> Result<bool> {
    let res = index_db
        .prepare("UPDATE sessions SET revoked_at = datetime('now') WHERE id = ?1 AND user_id = ?2 AND revoked_at IS NULL")
        .bind(&[session_id.into(), user_id.into()])?
        .run().await?;
    Ok(res.meta()?.and_then(|m| m.changes).unwrap_or(0) > 0)
}

pub async fn revoke_other_sessions(
    index_db: &D1Database,
    user_id: &str,
    keep_session_id: &str,
) -> Result<i64> {
    let res = index_db
        .prepare("UPDATE sessions SET revoked_at = datetime('now') WHERE user_id = ?1 AND id != ?2 AND revoked_at IS NULL")
        .bind(&[user_id.into(), keep_session_id.into()])?
        .run().await?;
    Ok(res.meta()?.and_then(|m| m.changes).unwrap_or(0) as i64)
}

pub async fn touch_session_last_seen(index_db: &D1Database, session_id: &str) -> Result<()> {
    let _ = index_db
        .prepare("UPDATE sessions SET last_seen_at = datetime('now') WHERE id = ?1")
        .bind(&[session_id.into()])?
        .run()
        .await?;
    Ok(())
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct WorkspaceRef {
    pub slug: String,
    pub name: Option<String>,
    pub role: String,
    pub platform: String,
    pub is_dm: bool,
    pub archived: bool,
}

pub async fn list_user_workspaces_with_names(
    index_db: &D1Database,
    user_id: &str,
) -> Result<Vec<WorkspaceRef>> {
    #[derive(Deserialize)]
    struct Row {
        slug: String,
        name: Option<String>,
        role: String,
        platform: String,
        is_dm: i64,
        archived_at: Option<String>,
    }
    let res = index_db
        .prepare(
            "SELECT w.slug, w.name, uw.role, w.platform, w.is_dm, w.archived_at \
         FROM user_workspaces uw JOIN workspaces_meta w ON w.slug = uw.workspace_slug \
         WHERE uw.user_id = ?1 ORDER BY w.created_at DESC",
        )
        .bind(&[user_id.into()])?
        .all()
        .await?;
    let rows: Vec<Row> = res.results()?;
    Ok(rows
        .into_iter()
        .map(|r| WorkspaceRef {
            slug: r.slug,
            name: r.name,
            role: r.role,
            platform: r.platform,
            is_dm: r.is_dm != 0,
            archived: r.archived_at.is_some(),
        })
        .collect())
}

pub async fn update_workspace_name(index_db: &D1Database, slug: &str, name: &str) -> Result<()> {
    index_db
        .prepare("UPDATE workspaces_meta SET name = ?2 WHERE slug = ?1")
        .bind(&[slug.into(), name.into()])?
        .run()
        .await?;
    Ok(())
}

pub async fn archive_workspace(index_db: &D1Database, slug: &str) -> Result<()> {
    index_db
        .prepare("UPDATE workspaces_meta SET archived_at = datetime('now') WHERE slug = ?1 AND archived_at IS NULL")
        .bind(&[slug.into()])?
        .run().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    // Compile-check only — these helpers need a real D1Database at runtime,
    // which is not available off-wasm. Actual behaviour verified in the
    // integration smoke test (scripts/test_auth_flow.sh) and in production
    // via wrangler tail.
    use super::*;

    #[allow(dead_code)]
    fn _lookup_user_by_identity_signature(
        db: &D1Database,
    ) -> impl std::future::Future<Output = Result<Option<String>>> + '_ {
        lookup_user_by_identity(db, "telegram", "12345")
    }
}
