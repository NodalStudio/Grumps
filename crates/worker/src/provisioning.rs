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

pub async fn provision_workspace(d1_client: &D1RestClient, index_db: &D1Database, platform: &str, channel_id: &str) -> Result<(String, String)> {
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
    index_db.prepare("INSERT INTO workspaces_meta (slug, platform, platform_channel_id, d1_database_id) VALUES (?1, ?2, ?3, ?4)")
        .bind(&[slug.clone().into(), platform.into(), channel_id.into(), database_id.clone().into()])?.run().await?;
    Ok((slug, database_id))
}
