//! Initial run and node rows written when a run is queued.

use serde_json::json;
use sqlbuild_recall::records::main::sha256_hex::sha256_hex;

use crate::runs::models::RunRequest;
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn project_queued_run(
    mut transaction: ControlTransaction,
    request: &RunRequest,
    cursor: i64,
) -> Result<ControlTransaction, ControlError> {
    let now = transaction.now;
    let selection = serde_json::to_value(&request.selection)
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let digest = sha256_hex(
        &serde_json::to_vec(&request.write_set)
            .map_err(|error| ControlError::Invalid(error.to_string()))?,
    );
    (transaction, _) = query(transaction,
        "INSERT INTO sqb_runs (run_id, location_id, target, snapshot_id, image_id, kind, origin, job_key, parent_run_id, root_run_id, trigger_id, tick_id, rerun_of_run_id, \
         requested_by, priority, selection_json, write_set_digest, status, queued_at, node_counts_json, last_event_storage_id, request_json) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,'queued',$18,$19,$20,$21) \
         ON CONFLICT (run_id) DO UPDATE SET location_id=excluded.location_id,target=excluded.target,snapshot_id=excluded.snapshot_id, \
         image_id=excluded.image_id,kind=excluded.kind,origin=excluded.origin,job_key=excluded.job_key,parent_run_id=excluded.parent_run_id, \
         root_run_id=excluded.root_run_id,trigger_id=excluded.trigger_id,tick_id=excluded.tick_id,rerun_of_run_id=excluded.rerun_of_run_id, \
         requested_by=excluded.requested_by,priority=excluded.priority,selection_json=excluded.selection_json,write_set_digest=excluded.write_set_digest, \
         status=excluded.status,queued_at=excluded.queued_at,node_counts_json=excluded.node_counts_json,last_event_storage_id=excluded.last_event_storage_id, \
         request_json=excluded.request_json,rewrite_history=FALSE,status_reason=NULL,launched_at=NULL,started_at=NULL,ended_at=NULL,cancel_requested_at=NULL",
        vec![Parameter::Id(request.run_id), Parameter::Id(request.location_id), Parameter::Text(request.target.clone()), Parameter::Id(request.snapshot_id),
            Parameter::OptionalText(request.image_id.clone()), Parameter::Text(request.kind.as_str().into()), Parameter::Text(request.origin.as_str().into()),
            Parameter::OptionalText(request.job_key.clone()), Parameter::OptionalId(request.parent_run_id), Parameter::Id(request.root_run_id),
            Parameter::OptionalId(request.trigger_id), Parameter::OptionalId(request.tick_id), Parameter::OptionalId(request.rerun_of_run_id),
            Parameter::Text(request.requested_by.clone()), Parameter::Integer(request.priority), Parameter::Json(selection), Parameter::Text(digest),
            Parameter::Time(now), Parameter::Json(json!({"pending": request.selection.len()})), Parameter::Integer(cursor), Parameter::Json(json!(request))]).await?;
    for node in &request.selection {
        (transaction, _) = query(transaction,
            "INSERT INTO sqb_run_nodes (run_id, node_key, status, writes) VALUES ($1,$2,'pending',$3)",
            vec![Parameter::Id(request.run_id), Parameter::Text(node.clone()), Parameter::Boolean(request.write_set.contains(node))]).await?;
    }
    Ok(transaction)
}
