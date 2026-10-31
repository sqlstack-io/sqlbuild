//! Validate and publish a frozen request inside the caller's audit transaction.

use std::collections::BTreeSet;

use serde_json::json;

use crate::runs::main::queue_run::queue_run;
use crate::runs::models::RunRequest;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn publish_request(
    transaction: ControlTransaction,
    request: &RunRequest,
) -> Result<(ControlTransaction, bool), ControlError> {
    if request.requested_by.trim().is_empty()
        || request.selection.is_empty()
        || !request.write_set.is_subset(&request.selection)
    {
        return Err(ControlError::Invalid(
            "run needs an actor, selection and a contained write set".into(),
        ));
    }
    let (transaction, previous) = query(
        transaction,
        "SELECT request_json FROM sqb_runs WHERE run_id=$1",
        vec![Parameter::Id(request.run_id)],
    )
    .await?;
    if let Some(previous) = previous.first() {
        if previous.get("request_json") != Some(&json!(request)) {
            return Err(ControlError::Invalid(
                "run ID already has different request content".into(),
            ));
        }
        return Ok((transaction, false));
    }
    let (transaction, nodes) = query(
        transaction,
        "SELECT node_key FROM sqb_snapshot_nodes WHERE snapshot_id=$1 AND EXISTS \
         (SELECT 1 FROM sqb_snapshots WHERE snapshot_id=$1 AND location_id=$2 AND target=$3 \
         AND image_id IS NOT DISTINCT FROM $4)",
        vec![
            Parameter::Id(request.snapshot_id),
            Parameter::Id(request.location_id),
            Parameter::Text(request.target.clone()),
            Parameter::OptionalText(request.image_id.clone()),
        ],
    )
    .await?;
    let keys: BTreeSet<&str> = nodes
        .iter()
        .filter_map(|row| row.get("node_key").and_then(serde_json::Value::as_str))
        .collect();
    if request
        .selection
        .iter()
        .any(|key| !keys.contains(key.as_str()))
    {
        return Err(ControlError::Invalid(
            "run selection or image does not match the pinned location and target snapshot".into(),
        ));
    }
    Ok((queue_run(transaction, request).await?, true))
}
