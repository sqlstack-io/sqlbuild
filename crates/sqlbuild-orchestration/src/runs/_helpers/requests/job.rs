//! Freeze execution inputs from one published job or group in the caller's write transaction.

use crate::credentials::models::AuditContext;
use crate::declarations::models::JobDefinition;
use crate::runs::main::job_write_set::job_write_set;
use crate::runs::models::{JobRunRequest, RunKind, RunOrigin, RunRequest};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn frozen_job_request(
    transaction: ControlTransaction,
    context: &AuditContext,
    request: &JobRunRequest,
) -> Result<(ControlTransaction, RunRequest), ControlError> {
    let (transaction, jobs) = query(
        transaction,
        "SELECT j.snapshot_id, j.definition_json, j.kind, s.image_id FROM sqb_jobs j \
         JOIN sqb_current_snapshots c ON c.location_id=j.location_id AND c.target=j.target AND c.snapshot_id=j.snapshot_id \
         JOIN sqb_snapshots s ON s.snapshot_id=j.snapshot_id WHERE j.location_id=$1 AND j.target=$2 AND j.job_key=$3",
        vec![Parameter::Id(request.location_id), Parameter::Text(request.target.clone()), Parameter::Text(request.job_key.clone())],
    ).await?;
    let row = jobs.first().ok_or_else(|| {
        ControlError::Invalid("published job not found in this location and target".into())
    })?;
    let snapshot_id = serde_json::from_value(row["snapshot_id"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let definition: JobDefinition = serde_json::from_value(row["definition_json"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let image_id = serde_json::from_value(row["image_id"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let kind: RunKind = serde_json::from_value(row["kind"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let (transaction, write_set) =
        job_write_set(transaction, snapshot_id, &definition.selection).await?;
    Ok((
        transaction,
        RunRequest {
            run_id: request.run_id,
            location_id: request.location_id,
            target: request.target.clone(),
            snapshot_id,
            image_id,
            kind,
            origin: RunOrigin::Manual,
            job_key: Some(request.job_key.clone()),
            parent_run_id: None,
            root_run_id: request.run_id,
            trigger_id: None,
            tick_id: None,
            rerun_of_run_id: None,
            requested_by: context.actor.clone(),
            priority: request.priority,
            selection: definition.selection,
            write_set: if kind == RunKind::Group {
                Default::default()
            } else {
                write_set
            },
            settings: definition.settings,
            concurrency_groups: definition.concurrency_groups,
            overlap: definition.overlap,
        },
    ))
}
