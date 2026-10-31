//! Retry immutable execution inputs, narrowing only to durable failed and skipped outcomes.

use std::collections::BTreeSet;

use serde_json::from_value;

use crate::credentials::models::AuditContext;
use crate::runs::models::{RerunRequest, RunOrigin, RunRequest, RunStatus};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn frozen_rerun(
    transaction: ControlTransaction,
    context: &AuditContext,
    request: &RerunRequest,
) -> Result<(ControlTransaction, RunRequest), ControlError> {
    let (mut transaction, rows) = query(
        transaction,
        "SELECT request_json, status FROM sqb_runs WHERE run_id=$1",
        vec![Parameter::Id(request.source_run_id)],
    )
    .await?;
    let source = rows
        .first()
        .ok_or_else(|| ControlError::Invalid("source run was not found".into()))?;
    let status: RunStatus = from_value(source["status"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    if !status.is_terminal() {
        return Err(ControlError::Invalid(
            "rerun requires a terminal source run".into(),
        ));
    }
    let mut frozen: RunRequest = from_value(source["request_json"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    if request.from_failure {
        let rows;
        (transaction, rows) = query(
            transaction,
            "SELECT node_key FROM sqb_run_nodes WHERE run_id=$1 AND status IN ('failed','skipped')",
            vec![Parameter::Id(request.source_run_id)],
        )
        .await?;
        let failed: BTreeSet<_> = rows
            .iter()
            .filter_map(|row| row["node_key"].as_str())
            .collect();
        frozen.selection.retain(|key| failed.contains(key.as_str()));
        frozen
            .write_set
            .retain(|key| frozen.selection.contains(key));
        if frozen.selection.is_empty() {
            return Err(ControlError::Invalid(
                "source run has no failed or skipped nodes".into(),
            ));
        }
    }
    frozen.run_id = request.run_id;
    frozen.origin = RunOrigin::Rerun;
    frozen.parent_run_id = None;
    frozen.root_run_id = request.run_id;
    frozen.trigger_id = None;
    frozen.tick_id = None;
    frozen.rerun_of_run_id = Some(request.source_run_id);
    frozen.requested_by = context.actor.clone();
    frozen.priority = request.priority;
    Ok((transaction, frozen))
}
