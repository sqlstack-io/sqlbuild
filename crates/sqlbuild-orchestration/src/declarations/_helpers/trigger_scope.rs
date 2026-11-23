//! A trigger override's permission scope is every node a schedule's job or group writes, or every node of the target for a sensor, and to pause or resume a schedule whose job can't be read.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::authorization::main::decisions::refuse_unknown::refuse_unknown;
use crate::credentials::models::{Action, AuditContext, PermissionScope};
use crate::declarations::models::{JobDefinition, TriggerChange, TriggerChangeRequest};
use crate::runs::main::requests::frozen_job_request::frozen_job_request;
use crate::runs::models::JobRunRequest;
use crate::snapshots::main::write_sets::job_write_set::job_write_set;
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
        TriggerChange::SetCursor { .. }
        | TriggerChange::ResetCursor
        | TriggerChange::SkipEvent { .. } => Action::SensorResetCursor,
    };
    let (transaction, rows) = query(transaction,
        "SELECT t.location_id,t.target,t.job_key,j.definition_json AS job_definition FROM sqb_triggers t \
         LEFT JOIN sqb_current_snapshots c ON c.location_id=t.location_id AND c.target=t.target \
         LEFT JOIN sqb_jobs j ON j.location_id=t.location_id AND j.target=t.target AND j.job_key=t.job_key AND j.snapshot_id=c.snapshot_id \
         WHERE t.trigger_id=$1 AND t.removed_in_snapshot_id IS NULL",
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
    let location_id: Uuid = serde_json::from_value(row["location_id"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let target: String = row["target"].as_str().unwrap_or_default().into();
    let without_job: bool = matches!(request.change, TriggerChange::Enable | TriggerChange::Pause)
        && !row["job_definition"].is_null()
        && serde_json::from_value::<JobDefinition>(row["job_definition"].clone()).is_err();
    let Some(job_key) = row["job_key"].as_str().filter(|_| !without_job) else {
        let (transaction, nodes) = target_nodes(transaction, (location_id, &target)).await?;
        return Ok((
            transaction,
            PermissionScope {
                action,
                location_id: Some(location_id),
                target: Some(target),
                nodes,
            },
        ));
    };
    let job = JobRunRequest {
        run_id: Uuid::nil(),
        location_id,
        target,
        job_key: job_key.into(),
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

/// Every node the target's current snapshot publishes: a sensor may start any of its jobs, and a job that can't be read may write any of its nodes.
async fn target_nodes(
    transaction: ControlTransaction,
    (location_id, target): (Uuid, &str),
) -> Result<(ControlTransaction, BTreeSet<String>), ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT n.node_key FROM sqb_snapshot_nodes n JOIN sqb_current_snapshots c \
         ON c.snapshot_id=n.snapshot_id WHERE c.location_id=$1 AND c.target=$2",
        vec![Parameter::Id(location_id), Parameter::Text(target.into())],
    )
    .await?;
    Ok((
        transaction,
        rows.iter()
            .filter_map(|row| row["node_key"].as_str().map(str::to_owned))
            .collect(),
    ))
}
