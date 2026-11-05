//! Freeze execution inputs from one published job or group in the caller's write transaction.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::declarations::main::frozen_definitions::frozen_definitions;
use crate::runs::models::{RunKind, RunOrigin, RunRequest};
use crate::snapshots::main::job_write_set::job_write_set;
use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;

pub(crate) async fn frozen_group_children(
    transaction: ControlTransaction,
    request: &RunRequest,
) -> Result<(ControlTransaction, Vec<RunRequest>), ControlError> {
    let (transaction, definitions) = frozen_definitions(transaction, request.snapshot_id).await?;
    let group = request
        .job_key
        .as_ref()
        .and_then(|name| definitions.groups.get(name))
        .ok_or_else(|| ControlError::Invalid("group is absent from the pinned snapshot".into()))?;
    let selection_matches = if request.origin == RunOrigin::Rerun {
        request.selection.is_subset(&group.job.selection)
    } else {
        group.job.selection == request.selection
    };
    if !selection_matches || !request.write_set.is_empty() {
        return Err(ControlError::Invalid(
            "group scope differs from its pinned membership".into(),
        ));
    }
    let (transaction, writers) =
        job_write_set(transaction, request.snapshot_id, &request.selection).await?;
    let mut children: Vec<RunRequest> = Vec::new();
    for name in group.jobs.keys() {
        let job = &definitions.jobs[name];
        let selection: BTreeSet<_> = job
            .selection
            .intersection(&request.selection)
            .cloned()
            .collect();
        if selection.is_empty() {
            continue;
        }
        children.push(RunRequest {
            run_id: Uuid::new_v5(&request.run_id, name.as_bytes()),
            kind: RunKind::Job,
            job_key: Some(name.clone()),
            parent_run_id: Some(request.run_id),
            rerun_of_run_id: request
                .rerun_of_run_id
                .map(|id| Uuid::new_v5(&id, name.as_bytes())),
            write_set: selection.intersection(&writers).cloned().collect(),
            selection,
            settings: job.settings.clone(),
            concurrency_groups: job.concurrency_groups.clone(),
            ..request.clone()
        });
    }
    Ok((transaction, children))
}
