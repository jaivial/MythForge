//! Background scheduler for time-based automations.
//!
//! No cron daemon, no Redis: a tokio task polls the control plane for due
//! automations, exactly like Neural.Workspace's Postgres-backed queue
//! (SKIP LOCKED + lease). Automations created by a prompt start running on the
//! next tick â no restart.

use std::sync::atomic::{AtomicBool, Ordering};

use std::time::Duration;

use tokio::sync::Notify;

use crate::api::state::SharedState;
use crate::error::Result;
use crate::automations::engine;

/// How often the scheduler looks for due automations.
pub const TICK: Duration = Duration::from_secs(60);

static RUNNING: AtomicBool = AtomicBool::new(false);
static WAKE: once_cell::sync::Lazy<Notify> = once_cell::sync::Lazy::new(Notify::new);

/// Start the scheduler once per process.
pub fn start(state: SharedState) {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(TICK) => {}
                _ = WAKE.notified() => {}
            }
            if let Err(e) = tick(&state).await {
                tracing::warn!(error = %e, "automation scheduler tick failed");
            }
        }
    });
}

/// Wake the scheduler immediately (called when automations change).
pub async fn notify_changed(_state: &SharedState) {
    WAKE.notify_waiters();
}

/// One pass: claim and run every due automation.
pub async fn tick(state: &SharedState) -> Result<usize> {
    let due = engine::due_automations(state).await?;
    let mut ran = 0usize;
    for id in due {
        match engine::run_by_id_no_company(state, id).await {
            Ok(()) => ran += 1,
            Err(e) => tracing::warn!(error = %e, automation = %id, "automation run failed"),
        }
    }
    Ok(ran)
}

