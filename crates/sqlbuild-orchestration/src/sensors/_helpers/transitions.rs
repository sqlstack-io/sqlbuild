//! Failure health is derived from decisive executed facts, never a mutable health flag.

use std::collections::BTreeSet;

use crate::runs::models::NodeOutcome;
use crate::sensors::models::{NodeEventKind, SensorEvaluationRequest};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn matches_transition(
    transaction: ControlTransaction,
    evaluation: &SensorEvaluationRequest,
    (node_key, outcome, storage_id): (&str, NodeOutcome, i64),
    kinds: &BTreeSet<NodeEventKind>,
) -> Result<(ControlTransaction, bool), ControlError> {
    if outcome == NodeOutcome::Success && kinds.contains(&NodeEventKind::Success) {
        return Ok((transaction, true));
    }
    let kind = match outcome {
        NodeOutcome::Success => NodeEventKind::Recovered,
        NodeOutcome::Failed => NodeEventKind::Failed,
        _ => return Ok((transaction, false)),
    };
    if !kinds.contains(&kind) {
        return Ok((transaction, false));
    }
    let (transaction, rows) = query(
        transaction,
        "SELECT e.payload -> 'change' -> 'change' ->> 'outcome' AS outcome \
         FROM recall_events e JOIN sqb_runs r ON e.run_id=CAST(r.run_id AS TEXT) \
         WHERE e.producer='scheduler' AND e.event_type='run_node_finished' \
         AND e.invocation_id='scheduler/' || e.run_id AND e.storage_id<$1 \
         AND (e.payload -> 'change' -> 'change' ->> 'node_key')=$2 \
         AND (e.payload -> 'change' -> 'change' ->> 'outcome') IN ('success','failed') \
         AND r.location_id=$3 AND r.target=$4 AND r.kind<>'group' \
         AND EXISTS (SELECT 1 FROM recall_events s \
             WHERE s.invocation_id=e.invocation_id AND s.run_id=e.run_id \
             AND s.producer='scheduler' AND s.event_type='run_node_started' \
             AND s.storage_id<e.storage_id \
             AND (s.payload -> 'change' ->> 'claim_id')=(e.payload -> 'change' ->> 'claim_id') \
             AND (s.payload -> 'change' -> 'change' ->> 'node_key')=$2) \
         ORDER BY e.storage_id DESC LIMIT 1",
        vec![
            Parameter::Integer(storage_id),
            Parameter::Text(node_key.into()),
            Parameter::Id(evaluation.location_id),
            Parameter::Text(evaluation.target.clone()),
        ],
    )
    .await?;
    let failed = rows
        .first()
        .is_some_and(|row| row["outcome"] == NodeOutcome::Failed.as_str());
    Ok((transaction, (outcome == NodeOutcome::Success) == failed))
}
