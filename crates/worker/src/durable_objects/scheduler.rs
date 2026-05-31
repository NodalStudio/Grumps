//! WorkspaceScheduler : 1 instance per workspace.
//! Holds the next-due-alarm via state.storage().set_alarm().
//! See spec § 7.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use worker::*;

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
    fn new(state: State, env: Env) -> Self {
        Self { state, env }
    }

    async fn fetch(&self, mut req: Request) -> Result<Response> {
        let body: ScheduleRpc = req
            .json()
            .await
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
        let slug = self.state.id().name().unwrap_or_default();
        if slug.is_empty() {
            console_log!("WorkspaceScheduler.alarm: empty slug");
            return Response::ok("noop");
        }
        let now = chrono::Utc::now().to_rfc3339();
        let due = match resolve_due_actions(&self.env, &slug, &now).await {
            Ok(v) => v,
            Err(e) => {
                console_log!("alarm: resolve_due_actions error: {e}");
                return Response::ok("noop");
            }
        };
        for action in &due {
            // Lock first
            let locked = match resolve_lock_action(&self.env, &slug, &action.id).await {
                Ok(b) => b,
                Err(e) => {
                    console_log!("lock error: {e}");
                    false
                }
            };
            if !locked {
                continue;
            } // someone else got it
            match crate::scheduler_executor::execute_action(&self.env, &slug, action).await {
                Ok(()) => console_log!("executed action {}", action.id),
                Err(e) => console_log!("execute_action error for {}: {e}", action.id),
            }
        }
        // Re-arm next
        if let Err(e) = self.recompute_alarm().await {
            console_log!("recompute_alarm error: {e}");
        }
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
            Err(e) => {
                console_log!("recompute_alarm error: {e}");
                return Ok(());
            }
        };
        match next_iso {
            Some(iso) => {
                let dt = parse_iso(&iso)?;
                self.state
                    .storage()
                    .set_alarm(dt.timestamp_millis())
                    .await?;
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
    use crate::d1_rest::D1RestClient;
    use crate::db::{get_index_db, lookup_workspace_by_slug, WorkspaceDb};
    let index = get_index_db(env)?;
    let ws = lookup_workspace_by_slug(&index, slug)
        .await?
        .ok_or_else(|| Error::RustError(format!("workspace not found: {slug}")))?;
    let client = D1RestClient::from_env(env)?;
    let db = WorkspaceDb::new(&client, ws.d1_database_id);
    db.next_pending_trigger_at().await
}

async fn resolve_due_actions(
    env: &Env,
    slug: &str,
    now_iso: &str,
) -> Result<Vec<grumps_scheduler::ScheduledAction>> {
    use crate::d1_rest::D1RestClient;
    use crate::db::{get_index_db, lookup_workspace_by_slug, WorkspaceDb};
    let index = get_index_db(env)?;
    let ws = lookup_workspace_by_slug(&index, slug)
        .await?
        .ok_or_else(|| Error::RustError(format!("workspace not found: {slug}")))?;
    let client = D1RestClient::from_env(env)?;
    let db = WorkspaceDb::new(&client, ws.d1_database_id);
    db.list_due_actions(now_iso, 50).await
}

async fn resolve_lock_action(env: &Env, slug: &str, action_id: &str) -> Result<bool> {
    use crate::d1_rest::D1RestClient;
    use crate::db::{get_index_db, lookup_workspace_by_slug, WorkspaceDb};
    let index = get_index_db(env)?;
    let ws = lookup_workspace_by_slug(&index, slug)
        .await?
        .ok_or_else(|| Error::RustError(format!("workspace not found: {slug}")))?;
    let client = D1RestClient::from_env(env)?;
    let db = WorkspaceDb::new(&client, ws.d1_database_id);
    db.mark_action_firing(action_id).await
}
