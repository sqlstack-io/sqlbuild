//! Observe heartbeat loss under the scheduler fence without releasing a live worker's claims.

use chrono::TimeDelta;
use uuid::Uuid;

use crate::runs::_helpers::lifecycle::publish_change::publish_change;
use crate::runs::constants::{HEARTBEAT_TIMEOUT_SECONDS, MONITOR_BATCH_SIZE};
use crate::runs::models::{ChangingRun, RunChange, RunChangeRequest};
use crate::scheduler::main::check_lease::check_lease;
use crate::scheduler::models::SchedulerLease;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Serialized writers make heartbeat inspection and the resulting fact one atomic observation.
pub async fn monitor_runs(
    store: &ControlStore,
    lease: &SchedulerLease,
) -> Result<usize, ControlError> {
    let mut transaction = check_lease(begin(store, TransactionMode::Write).await?, lease).await?;
    let cutoff = transaction.now - TimeDelta::seconds(HEARTBEAT_TIMEOUT_SECONDS);
    let rows;
    (transaction, rows) = query(
        transaction,
        &format!(
            "SELECT r.run_id, r.status, r.cancel_requested_at, r.node_counts_json, \
         r.last_event_storage_id, l.worker_instance FROM sqb_runs r \
         JOIN sqb_run_liveness l ON l.run_id=r.run_id \
         WHERE r.status='running' AND l.last_heartbeat_at<$1 \
         ORDER BY l.last_heartbeat_at, r.run_id LIMIT {MONITOR_BATCH_SIZE}"
        ),
        vec![Parameter::Time(cutoff)],
    )
    .await?;
    let observed = rows.len();
    for row in rows {
        let run_id: Uuid = serde_json::from_value(row["run_id"].clone())
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        let previous: i64 = serde_json::from_value(row["last_event_storage_id"].clone())
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        let state: ChangingRun = decode_row(row)?;
        let request = RunChangeRequest {
            event_id: Uuid::new_v5(&run_id, format!("unresponsive/{previous}").as_bytes()),
            run_id,
            claim_id: state.worker_instance,
            change: RunChange::Unresponsive,
        };
        transaction = publish_change(transaction, &request, &state).await?;
    }
    commit(check_lease(transaction, lease).await?).await?;
    Ok(observed)
}
