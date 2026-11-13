//! A sensor run key gates queue publication within the cursor/outcome transaction.

use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::declarations::models::{Overlap, StoredJob};
use crate::runs::main::active_job_run::active_job_run;
use crate::runs::main::frozen_job_request::frozen_job_request;
use crate::runs::main::queue_run::queue_run;
use crate::runs::models::{JobRunRequest, RunOrigin};
use crate::sensors::models::{SensorEvaluationRequest, SensorRequests, SkippedRequest};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::main::query_records::query_records;
use crate::store::models::{ControlTransaction, Parameter};

/// Queue the run for `run_key` once, or record it skipped while a run of a `skip` job is active.
pub(crate) async fn queue_sensor_run(
    transaction: ControlTransaction,
    evaluation: &SensorEvaluationRequest,
    run_key: &str,
    requests: &mut SensorRequests,
) -> Result<ControlTransaction, ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT run_id FROM sqb_run_requests WHERE trigger_id=$1 AND run_key=$2",
        vec![
            Parameter::Id(evaluation.trigger_id),
            Parameter::Text(run_key.into()),
        ],
    )
    .await?;
    if let Some(row) = rows.into_iter().next() {
        requests.run_ids.push(
            serde_json::from_value(row["run_id"].clone())
                .map_err(|error| ControlError::Invalid(error.to_string()))?,
        );
        return Ok(transaction);
    }
    let job_key = &evaluation.definition.job_key;
    let (transaction, overlap) = job_overlap(transaction, evaluation).await?;
    let (transaction, active) = if overlap.is_queue() {
        (transaction, None)
    } else {
        active_job_run(
            transaction,
            (evaluation.location_id, &evaluation.target, job_key),
        )
        .await?
    };
    if let Some(active_run_id) = active {
        requests.skipped.push(SkippedRequest {
            run_key: run_key.into(),
            active_run_id,
        });
        return Ok(transaction);
    }
    let run_id = Uuid::new_v5(&evaluation.trigger_id, run_key.as_bytes());
    let context = AuditContext {
        actor: "system:sensor".into(),
        granting_role: "system:scheduler".into(),
        request_id: evaluation.evaluation_id,
        ip_address: None,
    };
    let authored = JobRunRequest {
        run_id,
        location_id: evaluation.location_id,
        target: evaluation.target.clone(),
        job_key: evaluation.definition.job_key.clone(),
        reason: "Sensor condition matched".into(),
        priority: 0,
        window: None,
        full_refresh: false,
        select: Vec::new(),
        exclude: Vec::new(),
    };
    let (transaction, mut frozen) =
        frozen_job_request(transaction, (&context, None), &authored).await?;
    if frozen.snapshot_id != evaluation.snapshot_id {
        return Err(ControlError::Fenced);
    }
    frozen.origin = RunOrigin::Sensor;
    frozen.trigger_id = Some(evaluation.trigger_id);
    let transaction = queue_run(transaction, &frozen).await?;
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_run_requests (trigger_id,run_key,evaluation_id,run_id) VALUES ($1,$2,$3,$4)",
        vec![Parameter::Id(evaluation.trigger_id), Parameter::Text(run_key.into()), Parameter::Id(evaluation.evaluation_id), Parameter::Id(run_id)]).await?;
    requests.run_ids.push(run_id);
    Ok(transaction)
}

/// The overlap the sensor's job declares in the evaluated snapshot, which the target currently publishes.
async fn job_overlap(
    transaction: ControlTransaction,
    evaluation: &SensorEvaluationRequest,
) -> Result<(ControlTransaction, Overlap), ControlError> {
    let (transaction, jobs): (_, Vec<StoredJob>) = query_records(
        transaction,
        "SELECT job_key, kind, snapshot_id, definition_json FROM sqb_jobs \
         WHERE location_id=$1 AND target=$2 AND job_key=$3 AND snapshot_id=$4",
        vec![
            Parameter::Id(evaluation.location_id),
            Parameter::Text(evaluation.target.clone()),
            Parameter::Text(evaluation.definition.job_key.clone()),
            Parameter::Id(evaluation.snapshot_id),
        ],
    )
    .await?;
    Ok((
        transaction,
        jobs.first()
            .map_or(Overlap::Queue, |job| job.definition_json.overlap),
    ))
}
