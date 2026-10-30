//! Publish request facts, queued facts and initial node projections in the caller's transaction.

use serde_json::json;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;
use uuid::Uuid;

use crate::runs::_helpers::events::append_run_event;
use crate::runs::models::RunRequest;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn queue_run(
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
