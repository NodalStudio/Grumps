//! WorkspaceScheduler : 1 instance per workspace.
//! Holds the next-due-alarm via state.storage().set_alarm().
//! See spec § 7.

use worker::*;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ScheduleRpc {
    /// Schedule a new action — DO updates alarm if earlier than current.
    Schedule { trigger_at: String },
    /// Force recompute next alarm from D1 (e.g. after delete/cancel).
    Reschedule,
    /// Clear the alarm (e.g. last action cancelled).
    Clear,
}

#[durable_object]
pub struct WorkspaceScheduler {
    state: State,
    env: Env,
}

impl DurableObject for WorkspaceScheduler {
    fn new(state: State, env: Env) -> Self { Self { state, env } }

    async fn fetch(&self, mut req: Request) -> Result<Response> {
        let body: ScheduleRpc = req.json().await
            .map_err(|e| Error::RustError(format!("bad rpc body: {e}")))?;
        match body {
            ScheduleRpc::Schedule { trigger_at } => {
                let new_at = parse_iso(&trigger_at)?;
                let current = self.state.storage().get_alarm().await?;
                let new_ms = new_at.timestamp_millis();
                let should_update = current.map(|c| new_ms < c).unwrap_or(true);
                if should_update {
                    self.state.storage().set_alarm(new_ms).await?;
                }
                Response::ok("scheduled")
            }
            ScheduleRpc::Reschedule => {
                self.recompute_alarm().await?;
                Response::ok("rescheduled")
            }
            ScheduleRpc::Clear => {
                self.state.storage().delete_alarm().await?;
                Response::ok("cleared")
            }
        }
    }

    async fn alarm(&self) -> Result<Response> {
        // T15 skeleton : log + try to recompute next alarm (no-op execute yet — comes in T17)
        console_log!("WorkspaceScheduler alarm fired (skeleton, no-op)");
        self.recompute_alarm().await?;
        Response::ok("fired")
    }
}

impl WorkspaceScheduler {
    async fn recompute_alarm(&self) -> Result<()> {
        let slug = self.state.id().name().unwrap_or_default();
        if slug.is_empty() {
            console_log!("WorkspaceScheduler: empty slug, skipping recompute");
            return Ok(());
        }
        let next_iso = match resolve_next_pending(&self.env, &slug).await {
            Ok(opt) => opt,
            Err(e) => { console_log!("recompute_alarm error: {e}"); return Ok(()); }
        };
        match next_iso {
            Some(iso) => {
                let dt = parse_iso(&iso)?;
                self.state.storage().set_alarm(dt.timestamp_millis()).await?;
            }
            None => {
                self.state.storage().delete_alarm().await?;
            }
        }
        Ok(())
    }
}

fn parse_iso(s: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| Error::RustError(format!("bad iso datetime '{s}': {e}")))
}

async fn resolve_next_pending(env: &Env, slug: &str) -> Result<Option<String>> {
    use crate::db::{get_index_db, lookup_workspace_by_slug, WorkspaceDb};
    use crate::d1_rest::D1RestClient;
    let index = get_index_db(env)?;
    let ws = lookup_workspace_by_slug(&index, slug).await?
        .ok_or_else(|| Error::RustError(format!("workspace not found: {slug}")))?;
    let client = D1RestClient::from_env(env)?;
    let db = WorkspaceDb::new(&client, ws.d1_database_id);
    db.next_pending_trigger_at().await
}
