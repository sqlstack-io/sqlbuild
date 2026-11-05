//! A completion is immutable; only the current unexpired claim may publish its result.

use serde_json::json;
use uuid::Uuid;

use crate::platform::main::read_location_work::read_location_work;
use crate::platform::main::require_instance::require_instance;
use crate::platform::models::{LocationWork, LocationWorkOutcome, LocationWorkResult};
use crate::sensors::constants::MAX_SENSOR_RUN_REQUESTS;
use crate::sensors::models::CustomSensorOutcome;
use crate::store::errors::ControlError;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub async fn complete_location_work(
    store: &ControlStore,
    (location_id, request_id): (Uuid, Uuid),
    result: &LocationWorkResult,
) -> Result<bool, ControlError> {
    let transaction = begin(store, TransactionMode::Write).await?;
    let (transaction, stored) = read_location_work(transaction, request_id).await?;
    let stored = stored
        .filter(|stored| stored.record.request_json.location_id == location_id)
        .ok_or_else(|| ControlError::Invalid("work is not assigned to this location".into()))?;
    let transaction = require_instance(
        transaction,
        (
            location_id,
            result.session_id,
            &stored.record.request_json.target,
        ),
    )
    .await?;
    if let Some(previous) = stored.record.result_json {
        if previous != *result {
            return Err(ControlError::Invalid(
                "location work already has a different result".into(),
            ));
        }
        commit(transaction).await?;
        return Ok(false);
    }
    if stored.claim_id != Some(result.claim_id)
        || stored.session_id != Some(result.session_id)
        || stored
            .claimed_until
            .is_none_or(|until| until <= transaction.now)
        || stored.record.expires_at <= transaction.now
    {
        return Err(ControlError::Fenced);
    }
    let valid = match (&stored.record.request_json.work, &result.outcome) {
        (_, LocationWorkOutcome::Superseded {})
        | (LocationWork::CheckConnection {}, LocationWorkOutcome::Connection { .. }) => true,
        (
            LocationWork::EvaluateSensor { .. },
            LocationWorkOutcome::SensorEvaluation { outcome },
        ) => match outcome {
            CustomSensorOutcome::Completed { run_keys, .. } => {
                run_keys.len() <= MAX_SENSOR_RUN_REQUESTS
                    && run_keys.iter().all(|key| !key.trim().is_empty())
            }
            CustomSensorOutcome::Skipped { reason, .. } => !reason.trim().is_empty(),
            CustomSensorOutcome::Failed | CustomSensorOutcome::TimedOut => true,
        },
        _ => false,
    };
    if !valid {
        return Err(ControlError::Invalid(
            "location result does not match its work".into(),
        ));
    }
    let now = transaction.now;
    let (transaction, _) = query(transaction,
        "UPDATE sqb_location_work SET result_json=$2, completed_at=$3 WHERE request_id=$1 AND completed_at IS NULL",
        vec![Parameter::Id(request_id), Parameter::Json(json!(result)), Parameter::Time(now)]).await?;
    commit(transaction).await?;
    Ok(true)
}
