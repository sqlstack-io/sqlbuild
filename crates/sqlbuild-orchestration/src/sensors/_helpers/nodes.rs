//! Successful rebuilds require earlier immutable start evidence from the same run claim.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::runs::models::{NodeOutcome, RunChange, RunChangeRequest};
use crate::sensors::_helpers::requests::queue_sensor_run;
use crate::sensors::models::{NodeSensorEvent, SensorCursor, SensorEvaluationRequest};
use crate::store::errors::ControlError;
use crate::store::main::decode_row::decode_row;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn consume_node_events(
    transaction: ControlTransaction,
    evaluation: &SensorEvaluationRequest,
    selection: &BTreeSet<String>,
) -> Result<(ControlTransaction, (SensorCursor, Vec<Uuid>)), ControlError> {
    let (mut transaction, rows) = query(
        transaction,
        "SELECT storage_id,event_id,run_id,payload FROM recall_events \
         WHERE producer='scheduler' AND event_type='run_node_finished' \
         AND invocation_id='scheduler/' || run_id AND storage_id>$1 \
         ORDER BY storage_id LIMIT 100",
        vec![Parameter::Integer(evaluation.cursor.event_storage_id)],
    )
    .await?;
    let mut cursor = evaluation.cursor.clone();
    let mut run_ids: Vec<Uuid> = Vec::new();
    for row in rows {
        let event: NodeSensorEvent = decode_row(row)?;
        cursor.event_storage_id = event.storage_id;
        let request: RunChangeRequest = serde_json::from_value(event.payload["change"].clone())
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        if request.run_id != event.run_id || request.event_id != event.event_id {
            return Err(ControlError::Invalid("node event identity mismatch".into()));
        }
        let RunChange::NodeFinished {
            node_key,
            outcome: NodeOutcome::Success,
            ..
        } = &request.change
        else {
            continue;
        };
        if !selection.contains(node_key) {
            continue;
        }
        let started;
        (transaction, started) =
            prior_start(transaction, evaluation, &request, event.storage_id).await?;
        if started {
            let run_id;
            (transaction, run_id) =
                queue_sensor_run(transaction, evaluation, &event.event_id.to_string()).await?;
            run_ids.push(run_id);
        }
    }
    Ok((transaction, (cursor, run_ids)))
}

async fn prior_start(
    transaction: ControlTransaction,
    evaluation: &SensorEvaluationRequest,
    request: &RunChangeRequest,
    storage_id: i64,
) -> Result<(ControlTransaction, bool), ControlError> {
    let (RunChange::NodeFinished { node_key, .. }, Some(claim_id)) =
        (&request.change, request.claim_id)
    else {
        return Ok((transaction, false));
    };
    let (transaction, rows) = query(
        transaction,
        "SELECT s.event_id FROM sqb_runs r JOIN recall_events s ON s.run_id=CAST(r.run_id AS TEXT) \
         WHERE s.invocation_id=$1 AND s.run_id=$2 AND s.producer='scheduler' \
         AND s.event_type='run_node_started' AND s.storage_id<$3 \
         AND (s.payload -> 'change' ->> 'claim_id')=$4 \
         AND (s.payload -> 'change' -> 'change' ->> 'node_key')=$5 \
         AND r.run_id=$6 AND r.location_id=$7 AND r.target=$8 AND r.kind<>'group' LIMIT 1",
        vec![
            Parameter::Text(format!("scheduler/{}", request.run_id)),
            Parameter::Text(request.run_id.to_string()),
            Parameter::Integer(storage_id),
            Parameter::Text(claim_id.to_string()),
            Parameter::Text(node_key.clone()),
            Parameter::Id(request.run_id),
            Parameter::Id(evaluation.location_id),
            Parameter::Text(evaluation.target.clone()),
        ],
    )
    .await?;
    Ok((transaction, !rows.is_empty()))
}
