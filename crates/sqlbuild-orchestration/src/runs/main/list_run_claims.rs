//! Recover ownership from active claims, not stale lifecycle events or elapsed heartbeat guesses.

use uuid::Uuid;

use crate::runs::constants::MONITOR_BATCH_SIZE;
use crate::runs::models::RunClaim;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, decode_row::decode_row, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// Keyset pages include claimed-but-unstarted work and never include a neighboring scope.
pub async fn list_run_claims(
    store: &ControlStore,
    (location_id, target): (Uuid, &str),
    after: Option<Uuid>,
) -> Result<Vec<RunClaim>, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, rows) = query(
        transaction,
        &format!(
            "SELECT l.worker_instance AS claim_id, r.request_json AS request \
         FROM sqb_run_liveness l JOIN sqb_runs r ON r.run_id=l.run_id \
         WHERE r.location_id=$1 AND r.target=$2 AND ($3 IS NULL OR r.run_id>$3) \
         ORDER BY r.run_id LIMIT {MONITOR_BATCH_SIZE}"
        ),
        vec![
            Parameter::Id(location_id),
            Parameter::Text(target.into()),
            Parameter::OptionalId(after),
        ],
    )
    .await?;
    let claims = rows
        .into_iter()
        .map(decode_row)
        .collect::<Result<Vec<_>, _>>()?;
    commit(transaction).await?;
    Ok(claims)
}
