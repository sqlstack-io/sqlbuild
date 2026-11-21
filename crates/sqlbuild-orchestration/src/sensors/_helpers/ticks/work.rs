//! A sensor's function runs on its location; only this location-scoped work crosses to it, and only data comes back.

use std::collections::BTreeSet;

use chrono::TimeDelta;
use serde_json::json;

use crate::platform::main::work::read_location_work::read_location_work;
use crate::platform::models::{LocationWork, LocationWorkOutcome, LocationWorkRequest};
use crate::sensors::_helpers::ticks::requests::{queue_sensor_run, unknown_job};
use crate::sensors::constants::SENSOR_WORK_GRACE_SECONDS;
use crate::sensors::models::{
    RequestedRun, SensorEvaluationOutcome, SensorEvaluationRequest, SensorFunctionOutcome,
    SensorRequests,
};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// The failure code of a tick whose function raised.
const FUNCTION_FAILED: &str = "sensor_function_failed";
/// The failure code of a tick whose function asked for a job the target doesn't publish.
const UNKNOWN_JOB: &str = "unknown_job";

pub(crate) async fn request_evaluation_work(
    transaction: ControlTransaction,
    evaluation: &SensorEvaluationRequest,
) -> Result<ControlTransaction, ControlError> {
    let request = LocationWorkRequest {
        request_id: evaluation.evaluation_id,
        location_id: evaluation.location_id,
        target: evaluation.target.clone(),
        reason: format!("Run sensor {}", evaluation.sensor),
        work: LocationWork::EvaluateSensor {
            evaluation: Box::new(evaluation.clone()),
        },
    };
    let expires_at = evaluation.requested_at
        + TimeDelta::seconds(
            i64::from(evaluation.definition.timeout_seconds.get()) + SENSOR_WORK_GRACE_SECONDS,
        );
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_location_work \
         (request_id,location_id,target,snapshot_id,generation,request_json,requested_by,requested_at,expires_at) \
         SELECT $1,$2,$3,snapshot_id,generation,$4,'system:sensor',$5,$6 FROM sqb_snapshots WHERE snapshot_id=$7",
        vec![Parameter::Id(request.request_id), Parameter::Id(request.location_id), Parameter::Text(request.target.clone()),
            Parameter::Json(json!(request)), Parameter::Time(evaluation.requested_at),
            Parameter::Time(expires_at), Parameter::Id(evaluation.snapshot_id)]).await?;
    Ok(transaction)
}

pub(crate) async fn read_evaluation_result(
    transaction: ControlTransaction,
    evaluation: &SensorEvaluationRequest,
) -> Result<(ControlTransaction, Option<SensorEvaluationOutcome>), ControlError> {
    let (transaction, work) = read_location_work(transaction, evaluation.evaluation_id).await?;
    let work = work.ok_or_else(|| ControlError::Invalid("sensor has no location work".into()))?;
    if work.record.result_json.is_none() {
        let expired = work.record.expires_at <= transaction.now;
        return Ok((
            transaction,
            expired.then_some(SensorEvaluationOutcome::Abandoned),
        ));
    }
    match work.record.result_json.map(|result| result.outcome) {
        Some(LocationWorkOutcome::SensorEvaluation { outcome }) => {
            apply_result(transaction, evaluation, outcome).await
        }
        Some(LocationWorkOutcome::Superseded {}) => {
            Ok((transaction, Some(SensorEvaluationOutcome::Abandoned)))
        }
        _ => Err(ControlError::Invalid(
            "sensor has an invalid location result".into(),
        )),
    }
}

/// The tick's outcome from what the function returned; a refused request queues nothing.
async fn apply_result(
    mut transaction: ControlTransaction,
    evaluation: &SensorEvaluationRequest,
    outcome: SensorFunctionOutcome,
) -> Result<(ControlTransaction, Option<SensorEvaluationOutcome>), ControlError> {
    let (custom, runs, skip_reason) = match outcome {
        SensorFunctionOutcome::Handled { cursor } => (cursor, Vec::new(), None),
        SensorFunctionOutcome::Skipped { cursor, reason } => (cursor, Vec::new(), Some(reason)),
        SensorFunctionOutcome::Requested { cursor, runs } => (cursor, runs, None),
        SensorFunctionOutcome::Failed { error } => {
            return Ok((
                transaction,
                Some(SensorEvaluationOutcome::Failed {
                    code: FUNCTION_FAILED.into(),
                    error: Some(error),
                }),
            ));
        }
        SensorFunctionOutcome::TimedOut => {
            return Ok((transaction, Some(SensorEvaluationOutcome::TimedOut)));
        }
    };
    let runs: Vec<RequestedRun> = distinct(runs);
    let unknown;
    (transaction, unknown) = unknown_job(transaction, evaluation, &runs).await?;
    if let Some(job) = unknown {
        return Ok((
            transaction,
            Some(SensorEvaluationOutcome::Failed {
                code: UNKNOWN_JOB.into(),
                error: Some(format!(
                    "RunRequest names {job}, which target {} doesn't publish as a job or job group",
                    evaluation.target
                )),
            }),
        ));
    }
    let mut cursor = evaluation
        .cursor_after
        .clone()
        .unwrap_or_else(|| evaluation.cursor.clone());
    if evaluation.event.is_none() {
        cursor.custom = custom;
    }
    let mut requests = SensorRequests::default();
    for run in &runs {
        (transaction, requests) = queue_sensor_run(transaction, evaluation, run, requests).await?;
    }
    Ok((
        transaction,
        Some(SensorEvaluationOutcome::Completed {
            cursor,
            run_ids: requests.run_ids,
            skipped: requests.skipped,
            skip_reason,
        }),
    ))
}

/// `runs` without repeated run keys, in the order the function returned them.
fn distinct(runs: Vec<RequestedRun>) -> Vec<RequestedRun> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    runs.into_iter()
        .filter(|run| seen.insert(run.run_key.clone()))
        .collect()
}
