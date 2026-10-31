//! Observe one run and its current claim using the same database-clock snapshot.

use uuid::Uuid;

use crate::runs::models::RunObservation;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, decode_row::decode_row, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub async fn observe_run(
    store: &ControlStore,
    run_id: Uuid,
) -> Result<RunObservation, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let now = transaction.now;
    let (transaction, rows) = query(transaction, "SELECT r.status, r.cancel_requested_at, l.claimed_at, l.last_heartbeat_at, $2 AS observed_at FROM sqb_runs r LEFT JOIN sqb_run_liveness l ON l.run_id=r.run_id WHERE r.run_id=$1", vec![Parameter::Id(run_id), Parameter::Time(now)]).await?;
    let state = decode_row(
        rows.into_iter()
            .next()
            .ok_or_else(|| ControlError::Invalid("launched run is missing".into()))?,
    )?;
    commit(transaction).await?;
    Ok(state)
}
