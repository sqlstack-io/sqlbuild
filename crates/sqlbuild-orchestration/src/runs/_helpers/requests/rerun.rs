//! Retry immutable execution inputs, narrowing only to durable failed and skipped outcomes.

use std::collections::BTreeSet;

use serde_json::from_value;
use uuid::Uuid;

use crate::authorization::main::authorize_person::authorize_person;
use crate::credentials::models::{Action, AuditContext, DefaultRole, PermissionScope};
use crate::runs::_helpers::requests::permissions::authorize_execution;
use crate::runs::models::{RerunRequest, RunOrigin, RunRequest, RunStatus};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn frozen_rerun(
    transaction: ControlTransaction,
    (context, credentials): (&AuditContext, Option<Uuid>),
    request: &RerunRequest,
) -> Result<(ControlTransaction, (AuditContext, RunRequest)), ControlError> {
    let (mut transaction, rows) = query(
        transaction,
        "SELECT request_json, status FROM sqb_runs WHERE run_id=$1",
        vec![Parameter::Id(request.source_run_id)],
    )
    .await?;
    let Some(source) = rows.first() else {
        if let Some(credentials) = credentials {
            authorize_person(
                transaction,
                (context, credentials),
                &PermissionScope::action(Action::RunRerun, None),
                DefaultRole::None,
            )
            .await?;
        }
        return Err(ControlError::Invalid("source run was not found".into()));
    };
    let status: RunStatus = from_value(source["status"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let mut frozen: RunRequest = from_value(source["request_json"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    if request.from_failure {
        let rows;
        (transaction, rows) = query(
            transaction,
            "SELECT node_key FROM sqb_run_nodes WHERE run_id=$1 AND kind='node' AND status IN ('failed','skipped')",
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
    let mut context = context.clone();
    if let Some(credentials) = credentials {
        (transaction, context) = authorize_execution(
            transaction,
            (&context, credentials),
            (&frozen, Action::RunRerun),
        )
        .await?;
        frozen.requested_by.clone_from(&context.actor);
    }
    if !status.is_terminal() {
        return Err(ControlError::Invalid(
            "rerun requires a terminal source run".into(),
        ));
    }
    if request.from_failure && frozen.selection.is_empty() {
        return Err(ControlError::Invalid(
            "source run has no failed or skipped nodes".into(),
        ));
    }
    Ok((transaction, (context, frozen)))
}
