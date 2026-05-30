use worker::*;
use crate::d1_rest::D1RestClient;

const WORKSPACE_SCHEMA_0001: &str = include_str!("../../../migrations/workspace/0001_init.sql");
const WORKSPACE_SCHEMA_0002: &str = include_str!("../../../migrations/workspace/0002_memory.sql");
const WORKSPACE_SCHEMA_0003: &str = include_str!("../../../migrations/workspace/0003_calendar.sql");
const WORKSPACE_SCHEMA_0004: &str = include_str!("../../../migrations/workspace/0004_scheduling.sql");
const WORKSPACE_SCHEMA_0007: &str = include_str!("../../../migrations/workspace/0007_quality_signals.sql");
const WORKSPACE_SCHEMA_0008: &str = include_str!("../../../migrations/workspace/0008_llm_calls.sql");
const WORKSPACE_SCHEMA_0009: &str = include_str!("../../../migrations/workspace/0009_member_locale.sql");

pub fn generate_slug() -> String {
    uuid::Uuid::new_v4().to_string().replace('-', "")[..8].to_string()
}

/// Backward-compat: callers that don't care about name/is_dm.
pub async fn provision_workspace(
    d1_client: &D1RestClient,
    index_db: &D1Database,
    platform: &str,
    channel_id: &str,
) -> Result<(String, String)> {
    provision_workspace_with_meta(d1_client, index_db, platform, channel_id, None, false).await
}

/// DM workspaces (chat.type == "private" on Telegram). The user is both the
/// channel and the sole admin; welcome message is the shorter DM variant.
pub async fn provision_workspace_dm(
    d1_client: &D1RestClient,
    index_db: &D1Database,
    platform: &str,
    channel_id: &str,
    default_name: Option<&str>,
) -> Result<(String, String)> {
    provision_workspace_with_meta(d1_client, index_db, platform, channel_id, default_name, true).await
}

pub async fn provision_workspace_with_meta(
    d1_client: &D1RestClient,
    index_db: &D1Database,
    platform: &str,
    channel_id: &str,
    name: Option<&str>,
    is_dm: bool,
) -> Result<(String, String)> {
    let slug = generate_slug();
    let db_name = format!("grumps-ws-{}", slug);
    let database_id = d1_client.create_database(&db_name).await?;
    console_log!("Created D1 {} ({})", db_name, database_id);
    d1_client.exec_statements(&database_id, WORKSPACE_SCHEMA_0001).await?;
    d1_client.exec_statements(&database_id, WORKSPACE_SCHEMA_0002).await?;
    d1_client.exec_statements(&database_id, WORKSPACE_SCHEMA_0003).await?;
    d1_client.exec_statements(&database_id, WORKSPACE_SCHEMA_0004).await?;
    d1_client.exec_statements(&database_id, WORKSPACE_SCHEMA_0007).await?;
    d1_client.exec_statements(&database_id, WORKSPACE_SCHEMA_0008).await?;
    d1_client.exec_statements(&database_id, WORKSPACE_SCHEMA_0009).await?;
    index_db.prepare(
        "INSERT INTO workspaces_meta (slug, platform, platform_channel_id, name, d1_database_id, is_dm) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
    ).bind(&[
        slug.clone().into(),
        platform.into(),
        channel_id.into(),
        name.unwrap_or("").into(),
        database_id.clone().into(),
        (if is_dm { 1_i32 } else { 0_i32 }).into(),
    ])?.run().await?;

    // Seed workspace-scoped settings the agent reads when building its prompt.
    // `platform` is intrinsic and never changes; `workspace_name` may be refined
    // later by the platform's group-title update. Best-effort: a settings write
    // failure must not abort provisioning (the prompt falls back gracefully).
    let ws_db = crate::db::WorkspaceDb::new(d1_client, database_id.clone());
    ws_db.set_setting("platform", platform).await.ok();
    if let Some(n) = name.filter(|n| !n.is_empty()) {
        ws_db.set_setting("workspace_name", n).await.ok();
    }

    Ok((slug, database_id))
}
