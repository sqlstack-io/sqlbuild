//! Custom callbacks cross only the existing location-scoped outbound-work boundary.

use std::collections::BTreeSet;

use chrono::TimeDelta;
use serde_json::json;

use crate::platform::main::read_location_work::read_location_work;
use crate::platform::models::{LocationWork, LocationWorkOutcome, LocationWorkRequest};
use crate::sensors::_helpers::requests::queue_sensor_run;
use crate::sensors::models::SensorRequests;
use crate::sensors::models::{
    CustomSensorOutcome, SensorCondition, SensorEvaluationOutcome, SensorEvaluationRequest,
};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn request_evaluation_work(
    transaction: ControlTransaction,
    evaluation: &SensorEvaluationRequest,
) -> Result<ControlTransaction, ControlError> {
    if !matches!(evaluation.definition.when, SensorCondition::Custom { .. }) {
        return Ok(transaction);
    }
    let request = LocationWorkRequest {
        request_id: evaluation.evaluation_id,
        location_id: evaluation.location_id,
        target: evaluation.target.clone(),
        reason: "Evaluate custom sensor".into(),
        work: LocationWork::EvaluateSensor {
            evaluation: Box::new(evaluation.clone()),
        },
    };
    let (transaction, _) = query(transaction,
        "INSERT INTO sqb_location_work \
         (request_id,location_id,target,snapshot_id,generation,request_json,requested_by,requested_at,expires_at) \
         SELECT $1,$2,$3,snapshot_id,generation,$4,'system:sensor',$5,$6 FROM sqb_snapshots WHERE snapshot_id=$7",
        vec![Parameter::Id(request.request_id), Parameter::Id(request.location_id), Parameter::Text(request.target.clone()),
            Parameter::Json(json!(request)), Parameter::Time(evaluation.requested_at),
            Parameter::Time(evaluation.requested_at + TimeDelta::minutes(5)), Parameter::Id(evaluation.snapshot_id)]).await?;
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

async fn apply_result(
    mut transaction: ControlTransaction,
    evaluation: &SensorEvaluationRequest,
    outcome: CustomSensorOutcome,
) -> Result<(ControlTransaction, Option<SensorEvaluationOutcome>), ControlError> {
    let (custom, run_keys) = match outcome {
        CustomSensorOutcome::Completed { cursor, run_keys } => (cursor, run_keys),
        CustomSensorOutcome::Skipped { cursor, .. } => (cursor, Vec::new()),
        CustomSensorOutcome::Failed => {
            return Ok((
                transaction,
                Some(SensorEvaluationOutcome::Failed {
                    code: "custom_sensor_failed".into(),
                }),
            ));
        }
        CustomSensorOutcome::TimedOut => {
            return Ok((transaction, Some(SensorEvaluationOutcome::TimedOut)));
        }
    };
    let mut cursor = evaluation.cursor.clone();
    cursor.custom = custom;
    let mut requests = SensorRequests::default();
    let run_keys: BTreeSet<String> = run_keys.into_iter().collect();
    for key in run_keys {
        (transaction, requests) = queue_sensor_run(transaction, evaluation, &key, requests).await?;
    }
    Ok((
        transaction,
        Some(SensorEvaluationOutcome::Completed {
            cursor,
            run_ids: requests.run_ids,
            skipped: requests.skipped,
        }),
    ))
}
