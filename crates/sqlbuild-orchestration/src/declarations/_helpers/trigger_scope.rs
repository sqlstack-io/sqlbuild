//! A trigger override's permission scope is every node the trigger's current job or group writes.

use uuid::Uuid;

use crate::authorization::main::refuse_unknown::refuse_unknown;
use crate::credentials::models::{Action, AuditContext, PermissionScope};
use crate::declarations::models::{TriggerChange, TriggerChangeRequest};
use crate::runs::main::frozen_job_request::frozen_job_request;
use crate::runs::models::JobRunRequest;
use crate::snapshots::main::job_write_set::job_write_set;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// A group's scope is its frozen selection's write set, which covers every child's write set.
pub(crate) async fn trigger_scope(
    transaction: ControlTransaction,
    (context, credentials): (&AuditContext, Uuid),
    request: &TriggerChangeRequest,
) -> Result<(ControlTransaction, PermissionScope), ControlError> {
    let action = match request.change {
        TriggerChange::Enable | TriggerChange::Pause | TriggerChange::ResetDefault => {
            Action::TriggerToggle
        }
        TriggerChange::SetCursor { .. } | TriggerChange::ResetCursor => Action::SensorResetCursor,
    };
    let (transaction, rows) = query(transaction,
        "SELECT location_id,target,job_key FROM sqb_triggers WHERE trigger_id=$1 AND removed_in_snapshot_id IS NULL",
        vec![Parameter::Id(request.trigger_id)]).await?;
    let Some(row) = rows.first() else {
        return refuse_unknown(
            transaction,
            (context, credentials),
            (action, None),
            ControlError::Invalid("current trigger not found".into()),
        )
        .await;
    };
    let job = JobRunRequest {
        run_id: Uuid::nil(),
        location_id: serde_json::from_value(row["location_id"].clone())
            .map_err(|error| ControlError::Invalid(error.to_string()))?,
        target: row["target"].as_str().unwrap_or_default().into(),
        job_key: row["job_key"].as_str().unwrap_or_default().into(),
        reason: request.reason.clone(),
        priority: 0,
        window: None,
        full_refresh: false,
        select: Vec::new(),
        exclude: Vec::new(),
    };
    let (transaction, frozen) = frozen_job_request(transaction, (context, None), &job).await?;
    let (transaction, nodes) = job_write_set(
        transaction,
        frozen.snapshot_id,
        (&frozen.selection, &frozen.settings),
    )
    .await?;
    Ok((
        transaction,
        PermissionScope {
            action,
            location_id: Some(job.location_id),
            target: Some(job.target),
            nodes,
        },
    ))
}
