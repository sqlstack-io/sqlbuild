//! A callback gets its own interpreter; timeout and shutdown reap it before reporting a result.

use std::future::Future;
use std::path::Path;
use std::time::Duration;

use sqlbuild_orchestration::sensors::models::{CustomSensorOutcome, SensorEvaluationRequest};
use sqlbuild_orchestration::store::errors::ControlError;

use crate::native_commands::_helpers::execution::location_result;
use crate::native_commands::constants::{SENSOR_EVALUATION_SECONDS, SENSOR_RESULT_BYTES};
use crate::native_commands::models::LocationServe;

pub(super) async fn evaluate_location_sensor(
    options: &LocationServe,
    script: &Path,
    evaluation: &SensorEvaluationRequest,
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<CustomSensorOutcome>, ControlError> {
    let request =
        serde_json::to_vec(evaluation).map_err(|error| ControlError::Invalid(error.to_string()))?;
    let (mut child, reply) =
        match location_result((options, script), (request, SENSOR_RESULT_BYTES)) {
            Ok(exchange) => exchange,
            Err(_) => return Ok(Some(CustomSensorOutcome::Failed)),
        };
    tokio::pin!(stopped);
    let outcome = tokio::select! {
        result = &mut stopped => {
            child.kill().await?;
            result?;
            return Ok(None);
        },
        result = tokio::time::timeout(Duration::from_secs(SENSOR_EVALUATION_SECONDS), async {
            let outcome = reply.await?;
            if !child.wait().await?.success() {
                return Err(ControlError::Invalid("location sensor interpreter failed".into()));
            }
            Ok(outcome)
        }) => {
            match result {
                Ok(Ok(outcome)) => outcome,
                Ok(Err(_)) => CustomSensorOutcome::Failed,
                Err(_) => CustomSensorOutcome::TimedOut,
            }
        },
    };
    if child.try_wait()?.is_none() {
        child.kill().await?;
    }
    Ok(Some(outcome))
}
