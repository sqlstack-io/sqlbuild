//! One job's declaration needs the same view as the job list: every current node of the target.

use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::declarations::_helpers::job_detail::{
    critical_nodes, cursor_type, groups_running, members_in_order, shown_settings,
};
use crate::declarations::main::frozen_definitions::frozen_definitions;
use crate::declarations::main::published_job::published_job;
use crate::declarations::models::JobDetailRecord;
use crate::runs::models::RunKind;
use crate::snapshots::main::read_snapshot_nodes::read_snapshot_nodes;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

/// `None` when the target publishes no job by that name; refusal comes first either way.
pub async fn read_person_job(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    (location_id, target, job_key): (Uuid, &str, &str),
    default_role: DefaultRole,
) -> Result<Option<JobDetailRecord>, ControlError> {
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    let (transaction, published) =
        published_job(transaction, (location_id, target, job_key)).await?;
    let Some(snapshot_id) = published.map(|job| job.snapshot_id) else {
        commit(transaction).await?;
        return Ok(None);
    };
    let (transaction, definitions) = frozen_definitions(transaction, snapshot_id).await?;
    let (kind, job, members) = match (
        definitions.jobs.get(job_key),
        definitions.groups.get(job_key),
    ) {
        (Some(job), _) => (RunKind::Job, job.clone(), Vec::new()),
        (None, Some(group)) => (RunKind::Group, group.job.clone(), members_in_order(group)),
        (None, None) => {
            return Err(ControlError::Invalid(
                "published job is absent from its snapshot".into(),
            ));
        }
    };
    let (transaction, nodes) =
        read_snapshot_nodes(transaction, snapshot_id, Some(&job.selection)).await?;
    commit(transaction).await?;
    Ok(Some(JobDetailRecord {
        job_key: job_key.into(),
        kind,
        description: job.description,
        snapshot_id,
        settings: shown_settings(&job.settings),
        members,
        member_of: groups_running(&definitions, job_key),
        critical_nodes: critical_nodes(&nodes),
        cursor_type: cursor_type(&nodes),
        selection: job.selection,
        concurrency_groups: job.concurrency_groups,
        select: job.select,
        exclude: job.exclude,
        targets: job.targets,
        declared_at: job.declared_at,
        owner: job.owner,
        overlap: job.overlap,
        on_failure: job.on_failure,
    }))
}
