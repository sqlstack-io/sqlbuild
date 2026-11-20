//! A sensor's function gets its own interpreter; its timeout and shutdown reap it before reporting a result.

use std::future::Future;
use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use sqlbuild_orchestration::sensors::models::SensorFunctionOutcome;
use sqlbuild_orchestration::store::errors::ControlError;

use crate::native_commands::_helpers::location::execution::location_result;
use crate::native_commands::constants::SENSOR_RESULT_BYTES;
use crate::native_commands::models::LocationServe;

/// Run one call of a sensor's function for `request`, a tick or a trial; `None` when `stopped` ends it first.
pub(crate) async fn run_sensor_function(
    (options, script): (&LocationServe, &Path),
    (request, timeout_seconds): (&impl Serialize, u32),
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<SensorFunctionOutcome>, ControlError> {
    let request =
        serde_json::to_vec(request).map_err(|error| ControlError::Invalid(error.to_string()))?;
    let (mut child, reply) =
        match location_result((options, script), (request, SENSOR_RESULT_BYTES)) {
            Ok(exchange) => exchange,
            Err(error) => {
                return Ok(Some(SensorFunctionOutcome::Failed {
                    error: format!("the location couldn't start the sensor's interpreter: {error}"),
                }));
            }
        };
    tokio::pin!(stopped);
    let outcome = tokio::select! {
        result = &mut stopped => {
            child.kill().await?;
            result?;
            return Ok(None);
        },
        result = tokio::time::timeout(Duration::from_secs(u64::from(timeout_seconds)), async {
            let outcome = reply.await?;
            if !child.wait().await?.success() {
                return Err(ControlError::Invalid("the sensor's interpreter exited with an error".into()));
            }
            Ok(outcome)
        }) => {
            match result {
                Ok(Ok(outcome)) => outcome,
                Ok(Err(error)) => SensorFunctionOutcome::Failed { error: error.to_string() },
                Err(_) => SensorFunctionOutcome::TimedOut,
            }
        },
    };
    if child.try_wait()?.is_none() {
        child.kill().await?;
    }
    Ok(Some(outcome))
}
