//! Project retained facts without republishing events or touching live ownership.

use crate::runs::_helpers::lifecycle::projection::project_run_state;
use crate::runs::_helpers::lifecycle::queued::project_queued_run;
use crate::runs::models::{
    ChangingRun, RunChange, RunChangeRequest, RunClaim, RunReplayRow, RunRequest,
};
use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query};
use crate::store::models::{Change, ControlTransaction, Parameter};

pub(crate) async fn replay_run_event(
    mut transaction: ControlTransaction,
    row: RunReplayRow,
) -> Result<ControlTransaction, ControlError> {
    if row.schema_version != 1 {
        return Err(ControlError::Invalid(
            "cannot replay an unsupported run fact schema".into(),
        ));
    }
    transaction.now = row.occurred_at;
    match row.event_type.as_str() {
        "run_requested" => return Ok(transaction),
        "run_queued" => {
            let request: RunRequest = serde_json::from_value(row.payload["request"].clone())
                .map_err(|error| ControlError::Invalid(error.to_string()))?;
            if request.run_id != row.run_id {
                return Err(ControlError::Invalid(
                    "queued fact has a different run identity".into(),
                ));
            }
            (transaction, _) = query(
                transaction,
                "DELETE FROM sqb_run_nodes WHERE run_id=$1",
                vec![Parameter::Id(row.run_id)],
            )
            .await?;
            transaction = project_queued_run(transaction, &request, row.storage_id).await?;
        }
        "run_claimed" => {
            let claim: RunClaim = serde_json::from_value(row.payload["claim"].clone())
                .map_err(|error| ControlError::Invalid(error.to_string()))?;
            if claim.request.run_id != row.run_id {
                return Err(ControlError::Invalid(
                    "claim fact has a different run identity".into(),
                ));
            }
        }
        _ => {
            let request: RunChangeRequest =
                serde_json::from_value(row.payload["change"].clone())
                    .map_err(|error| ControlError::Invalid(error.to_string()))?;
            if request.run_id != row.run_id || request.change.event_type() != row.event_type {
                return Err(ControlError::Invalid(
                    "run fact identity or change type does not match".into(),
                ));
            }
            if matches!(request.change, RunChange::LaunchFailed { .. }) {
                return Ok(transaction);
            }
            let states;
            (transaction, states) = query(transaction,
                "SELECT status,cancel_requested_at,node_counts_json,NULL AS worker_instance FROM sqb_runs WHERE run_id=$1",
                vec![Parameter::Id(row.run_id)]).await?;
            let state: ChangingRun = decode_row(states.into_iter().next().ok_or_else(|| {
                ControlError::Invalid("run change has no retained queued request".into())
            })?)?;
            transaction = project_run_state(transaction, &request, &state).await?;
        }
    }
    let affected;
    (transaction, affected) = query(
        transaction,
        "UPDATE sqb_runs SET last_event_storage_id=$1 WHERE run_id=$2 RETURNING run_id",
        vec![
            Parameter::Integer(row.storage_id),
            Parameter::Id(row.run_id),
        ],
    )
    .await?;
    if affected.is_empty() {
        return Err(ControlError::Invalid(
            "run fact has no retained queued request".into(),
        ));
    }
    transaction.changes.insert(Change::Run(row.run_id));
    Ok(transaction)
}
