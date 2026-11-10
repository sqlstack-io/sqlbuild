//! Freeze published jobs for manual and sensor requests in their owning transaction.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::declarations::models::JobDefinition;
use crate::runs::_helpers::requests::adhoc::publication_plan;
use crate::runs::constants::FULL_REFRESH_SETTING;
use crate::runs::models::{
    JobRunRequest, RunCommand, RunKind, RunNarrowing, RunOrigin, RunRequest,
};
use crate::runs::types::CommandPlanner;
use crate::snapshots::main::job_write_set::job_write_set;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// Without a planner, a request that narrows the job is refused.
pub(crate) async fn frozen_job_request(
    transaction: ControlTransaction,
    (context, planner): (&AuditContext, Option<&dyn CommandPlanner>),
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
    let mut definition: JobDefinition = serde_json::from_value(row["definition_json"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let image_id = serde_json::from_value(row["image_id"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let kind: RunKind = serde_json::from_value(row["kind"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    if request.full_refresh {
        definition
            .settings
            .insert(FULL_REFRESH_SETTING.into(), serde_json::Value::Bool(true));
    }
    let mut transaction = transaction;
    let narrowing: Option<RunNarrowing> =
        (!request.select.is_empty() || !request.exclude.is_empty()).then(|| RunNarrowing {
            select: request.select.clone(),
            exclude: request.exclude.clone(),
        });
    if narrowing.is_some() {
        (transaction, definition.selection) = narrowed(
            transaction,
            (planner, snapshot_id, kind),
            (request, definition.selection),
        )
        .await?;
    }
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
            command: RunCommand::Build,
            origin: if request.window.is_some() {
                RunOrigin::Backfill
            } else {
                RunOrigin::Manual
            },
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
            window: request.window.clone(),
            concurrency_groups: definition.concurrency_groups,
            narrowing,
        },
    ))
}

/// The job's nodes that `sqb build --select … --exclude …` also selects; never a node outside the job.
async fn narrowed(
    transaction: ControlTransaction,
    (planner, snapshot_id, kind): (Option<&dyn CommandPlanner>, Uuid, RunKind),
    (request, selection): (&JobRunRequest, BTreeSet<String>),
) -> Result<(ControlTransaction, BTreeSet<String>), ControlError> {
    let invalid = |message: &str| Err(ControlError::Invalid(message.into()));
    if kind == RunKind::Group {
        return invalid("a job group cannot be narrowed; narrow one of its jobs");
    }
    let Some(planner) = planner else {
        return invalid("this request cannot narrow a job");
    };
    if request
        .select
        .iter()
        .chain(&request.exclude)
        .any(|token| token.trim().is_empty() || token.starts_with('-'))
    {
        return invalid("job selectors must be selector tokens, not flags");
    }
    let mut command: Vec<String> = vec!["build".into()];
    for (flag, tokens) in [
        ("--select", &request.select),
        ("--exclude", &request.exclude),
    ] {
        if !tokens.is_empty() {
            command.push(flag.into());
            command.extend(tokens.iter().cloned());
        }
    }
    let (transaction, plan) =
        publication_plan(transaction, (planner, snapshot_id), &command).await?;
    let plan = plan.map_err(ControlError::Invalid)?;
    let narrowed: BTreeSet<String> = selection.intersection(&plan.selection).cloned().collect();
    if narrowed.is_empty() {
        return invalid("the selectors leave none of the job's nodes");
    }
    Ok((transaction, narrowed))
}
