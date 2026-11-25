//! A plan can't wait for a location that can't load the current code: it fails with the load's error.

use serde_json::json;
use uuid::Uuid;

use crate::platform::models::{
    LocationHealth, LocationHealthObservation, LocationHealthReport, LocationWorkOutcome,
    LocationWorkResult,
};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// The plan error's code when the location's newest load failed.
pub(crate) const LOAD_FAILED_CODE: &str = "location_load_failed";
/// The error of a load from an older location, which reports no error text.
const UNREPORTED_LOAD_ERROR: &str = "the location's output has the error";

/// Why a plan can't run: the location's newest load failed, with its error.
pub(crate) fn load_failure_message(error: Option<&str>) -> String {
    format!(
        "the location can't load the current code, so it can't plan: {}",
        error.unwrap_or(UNREPORTED_LOAD_ERROR)
    )
}

/// The error of the newest failed load among `observations`, newest first.
pub(crate) fn newest_load_error(observations: &[LocationHealthObservation]) -> Option<&str> {
    observations
        .iter()
        .find_map(|observation| match &observation.report.health {
            LocationHealth::CompileFailed { error, .. } => Some(error.as_deref()),
            _ => None,
        })
        .flatten()
}

/// Finish every plan still waiting on the target with the failed load's error, as if the location had answered it.
pub(crate) async fn fail_waiting_plans(
    transaction: ControlTransaction,
    (location_id, target, session_id): (Uuid, &str, Uuid),
    error: Option<&str>,
) -> Result<ControlTransaction, ControlError> {
    let message = load_failure_message(error);
    let result = LocationWorkResult {
        claim_id: Uuid::nil(),
        session_id,
        outcome: LocationWorkOutcome::Plan {
            document: json!({"error": {"code": LOAD_FAILED_CODE, "message": message}}),
            diffs: Default::default(),
        },
    };
    let now = transaction.now;
    let (transaction, _) = query(
        transaction,
        "UPDATE sqb_location_work SET result_json=$4, completed_at=$5 \
         WHERE location_id=$1 AND target=$2 AND completed_at IS NULL AND expires_at>$3 \
         AND request_json->'work'->>'kind'='plan'",
        vec![
            Parameter::Id(location_id),
            Parameter::Text(target.to_owned()),
            Parameter::Time(now),
            Parameter::Json(json!(result)),
            Parameter::Time(now),
        ],
    )
    .await?;
    Ok(transaction)
}

/// `fail_waiting_plans` when `report` is a failed load; other reports leave plans waiting.
pub(crate) async fn fail_plans_on_failed_load(
    transaction: ControlTransaction,
    location_id: Uuid,
    report: &LocationHealthReport,
) -> Result<ControlTransaction, ControlError> {
    let LocationHealth::CompileFailed { error, .. } = &report.health else {
        return Ok(transaction);
    };
    fail_waiting_plans(
        transaction,
        (location_id, &report.target, report.session_id),
        error.as_deref(),
    )
    .await
}
