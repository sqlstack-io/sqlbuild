//! Publish request facts, queued facts and initial node projections in the caller's transaction.

use serde_json::json;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::declarations::main::frozen_definitions::frozen_definitions;
use crate::runs::_helpers::lifecycle::events::append_run_event;
use crate::runs::main::job_write_set::job_write_set;
use crate::runs::models::{RunKind, RunOrigin, RunRequest};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn queue_run(
    transaction: ControlTransaction,
    request: &RunRequest,
) -> Result<ControlTransaction, ControlError> {
    let mut transaction = queue_one(transaction, request).await?;
    if request.kind == RunKind::Group {
        let definitions;
        (transaction, definitions) = frozen_definitions(transaction, request.snapshot_id).await?;
        let group = request
            .job_key
            .as_ref()
            .and_then(|name| definitions.groups.get(name))
            .ok_or_else(|| {
                ControlError::Invalid("group is absent from the pinned snapshot".into())
            })?;
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
        let writers;
        (transaction, writers) =
            job_write_set(transaction, request.snapshot_id, &request.selection).await?;
        for name in group.jobs.keys() {
            let job = &definitions.jobs[name];
            let selection: std::collections::BTreeSet<_> = job
                .selection
                .intersection(&request.selection)
                .cloned()
                .collect();
            if selection.is_empty() {
                continue;
            }
            let child = RunRequest {
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
                overlap: job.overlap,
                ..request.clone()
            };
            transaction = queue_one(transaction, &child).await?;
        }
    }
    Ok(transaction)
}

async fn queue_one(
    mut transaction: ControlTransaction,
    request: &RunRequest,
) -> Result<ControlTransaction, ControlError> {
    let now = transaction.now;
    let mut cursor = None;
    for kind in ["run_requested", "run_queued"] {
        let storage_id;
        (transaction, storage_id) = append_run_event(
            transaction,
            (
                request.run_id,
                Uuid::new_v5(&request.run_id, kind.as_bytes()),
            ),
            kind,
            json!({"request": request}),
        )
        .await?;
        cursor = Some(storage_id);
    }
    let cursor =
        cursor.ok_or_else(|| ControlError::Invalid("queued event has no storage cursor".into()))?;
    let selection = serde_json::to_value(&request.selection)
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let digest = sha256_hex(
        &serde_json::to_vec(&request.write_set)
            .map_err(|error| ControlError::Invalid(error.to_string()))?,
    );
    (transaction, _) = query(transaction,
        "INSERT INTO sqb_runs (run_id, location_id, target, snapshot_id, image_id, kind, origin, job_key, parent_run_id, root_run_id, trigger_id, tick_id, rerun_of_run_id, \
         requested_by, priority, selection_json, write_set_digest, status, queued_at, node_counts_json, last_event_storage_id, request_json) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,'queued',$18,$19,$20,$21)",
        vec![Parameter::Id(request.run_id), Parameter::Id(request.location_id), Parameter::Text(request.target.clone()), Parameter::Id(request.snapshot_id),
            Parameter::OptionalText(request.image_id.clone()), Parameter::Text(request.kind.as_str().into()), Parameter::Text(request.origin.as_str().into()),
            Parameter::OptionalText(request.job_key.clone()), Parameter::OptionalId(request.parent_run_id), Parameter::Id(request.root_run_id),
            Parameter::OptionalId(request.trigger_id), Parameter::OptionalId(request.tick_id), Parameter::OptionalId(request.rerun_of_run_id),
            Parameter::Text(request.requested_by.clone()), Parameter::Integer(request.priority), Parameter::Json(selection), Parameter::Text(digest),
            Parameter::Time(now), Parameter::Json(json!({"pending": request.selection.len()})), Parameter::Integer(cursor), Parameter::Json(json!(request))]).await?;
    for node in &request.selection {
        (transaction, _) = query(transaction,
            "INSERT INTO sqb_run_nodes (run_id, node_key, attempt, status, writes) VALUES ($1,$2,1,'pending',$3)",
            vec![Parameter::Id(request.run_id), Parameter::Text(node.clone()), Parameter::Boolean(request.write_set.contains(node))]).await?;
    }
    Ok(transaction)
}
