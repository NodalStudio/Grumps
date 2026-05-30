//! Per-workspace D1 schema migrations: a versioned registry + an idempotent
//! runner.
//!
//! Replaces the ad-hoc `include_str!` / `exec_statements` block that ran at
//! provisioning time only. New workspaces are migrated via this runner; existing
//! workspace databases can be backfilled with `POST /api/admin/migrate-all`.
//!
//! To add a migration: drop a `migrations/workspace/NNNN_*.sql` file and append
//! one entry to `workspace_migrations()` (strictly increasing version — never
//! renumber or reuse). The runner records applied versions in `schema_migrations`.

use crate::d1_rest::{D1RestClient, extract_rows};
use worker::{Result, console_log};

pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

/// The ordered migration registry (version 6 was never created — that gap is
/// intentional and harmless).
pub fn workspace_migrations() -> Vec<Migration> {
    vec![
        Migration { version: 1, name: "init",              sql: include_str!("../../../migrations/workspace/0001_init.sql") },
        Migration { version: 2, name: "memory",            sql: include_str!("../../../migrations/workspace/0002_memory.sql") },
        Migration { version: 3, name: "calendar",          sql: include_str!("../../../migrations/workspace/0003_calendar.sql") },
        Migration { version: 4, name: "scheduling",        sql: include_str!("../../../migrations/workspace/0004_scheduling.sql") },
        Migration { version: 5, name: "migrate_reminders", sql: include_str!("../../../migrations/workspace/0005_migrate_reminders.sql") },
        Migration { version: 7, name: "quality_signals",   sql: include_str!("../../../migrations/workspace/0007_quality_signals.sql") },
        Migration { version: 8, name: "llm_calls",         sql: include_str!("../../../migrations/workspace/0008_llm_calls.sql") },
        Migration { version: 9, name: "member_locale",     sql: include_str!("../../../migrations/workspace/0009_member_locale.sql") },
    ]
}

async fn table_exists(client: &D1RestClient, db: &str, name: &str) -> Result<bool> {
    let resp = client.query(
        db,
        "SELECT name FROM sqlite_master WHERE type='table' AND name=?1",
        vec![name.into()],
    ).await?;
    Ok(!extract_rows::<serde_json::Value>(&resp)?.is_empty())
}

/// Apply every registered migration not yet recorded, in order. Idempotent.
///
/// A database provisioned before migration-tracking existed has the full schema
/// but no `schema_migrations` table. Re-running its (ALTER-based) migrations
/// would fail, so such a database is **baselined**: the current registry is
/// recorded as applied without re-running. Genuinely new (empty) databases get
/// every migration applied. Returns the versions newly applied.
pub async fn apply_pending(client: &D1RestClient, database_id: &str) -> Result<Vec<u32>> {
    client.exec_statements(
        database_id,
        "CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, applied_at TEXT DEFAULT (datetime('now')));",
    ).await?;

    #[derive(serde::Deserialize)]
    struct V { version: u32 }
    let resp = client.query(database_id, "SELECT version FROM schema_migrations", vec![]).await?;
    let applied: std::collections::HashSet<u32> =
        extract_rows::<V>(&resp)?.into_iter().map(|v| v.version).collect();

    let registry = workspace_migrations();

    // Pre-tracking database: schema present, nothing recorded → baseline.
    if applied.is_empty() && table_exists(client, database_id, "todos").await? {
        for m in &registry {
            client.query(
                database_id,
                "INSERT OR IGNORE INTO schema_migrations (version, name) VALUES (?1, ?2)",
                vec![m.version.into(), m.name.into()],
            ).await?;
        }
        console_log!(
            "[migration] baselined existing db {} at v{}",
            database_id,
            registry.last().map(|m| m.version).unwrap_or(0)
        );
        return Ok(vec![]);
    }

    let mut newly = vec![];
    for m in &registry {
        if applied.contains(&m.version) { continue; }
        client.exec_statements(database_id, m.sql).await?;
        client.query(
            database_id,
            "INSERT OR IGNORE INTO schema_migrations (version, name) VALUES (?1, ?2)",
            vec![m.version.into(), m.name.into()],
        ).await?;
        console_log!("[migration] applied v{} ({}) -> {}", m.version, m.name, database_id);
        newly.push(m.version);
    }
    Ok(newly)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_strictly_increasing() {
        let m = workspace_migrations();
        assert!(!m.is_empty());
        for w in m.windows(2) {
            assert!(w[1].version > w[0].version, "migration versions must strictly increase");
        }
    }

    #[test]
    fn all_have_sql_and_name() {
        for m in workspace_migrations() {
            assert!(!m.sql.trim().is_empty(), "migration v{} has empty SQL", m.version);
            assert!(!m.name.is_empty(), "migration v{} has empty name", m.version);
        }
    }
}
