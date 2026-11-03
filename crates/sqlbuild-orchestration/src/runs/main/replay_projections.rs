//! Reconstruct run and node projections within the caller's serialized source cut.

use crate::runs::_helpers::lifecycle::replay::replay_run_event;
use crate::runs::models::RunReplayRow;
use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query};
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn replay_run_projections(
    transaction: ControlTransaction,
) -> Result<ControlTransaction, ControlError> {
    let (mut transaction, unsupported) = query(transaction,
        "SELECT r.run_id FROM sqb_runs r WHERE NOT EXISTS (SELECT 1 FROM recall_events e \
         WHERE e.producer='scheduler' AND e.event_type='run_queued' AND e.run_id=CAST(r.run_id AS TEXT) \
         AND e.invocation_id='scheduler/' || e.run_id) LIMIT 1", vec![]).await?;
    if !unsupported.is_empty() {
        return Err(ControlError::Invalid(
            "run projection has no retained queued source fact".into(),
        ));
    }
    let mut after = 0;
    loop {
        let rows;
        (transaction, rows) = query(transaction,
            "SELECT storage_id,event_schema_version AS schema_version,occurred_at,event_type,run_id,payload FROM recall_events \
             WHERE producer='scheduler' AND invocation_id='scheduler/' || run_id AND storage_id>$1 \
             ORDER BY storage_id LIMIT 100", vec![Parameter::Integer(after)]).await?;
        if rows.is_empty() {
            break;
        }
        for row in rows {
            let row: RunReplayRow = decode_row(row)?;
            after = row.storage_id;
            transaction = replay_run_event(transaction, row).await?;
        }
    }
    Ok(transaction)
}
