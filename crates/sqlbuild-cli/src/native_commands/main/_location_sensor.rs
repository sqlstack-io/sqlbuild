//! A callback gets its own interpreter; timeout and shutdown reap it before reporting a result.

use std::future::Future;
use std::path::Path;
use std::time::Duration;

use sqlbuild_orchestration::sensors::models::{CustomSensorOutcome, SensorEvaluationRequest};
use sqlbuild_orchestration::store::errors::ControlError;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Child;

use crate::native_commands::_helpers::execution::location_runtime_child;
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
    let mut child = match location_runtime_child(options, script) {
        Ok(child) => child,
        Err(_) => return Ok(Some(CustomSensorOutcome::Failed)),
    };
    tokio::pin!(stopped);
    let outcome = tokio::select! {
        result = &mut stopped => {
            child.kill().await?;
            result?;
            return Ok(None);
        },
        result = tokio::time::timeout(Duration::from_secs(SENSOR_EVALUATION_SECONDS), exchange_result(&mut child, request)) => {
            match result {
                Ok(Ok((_, outcome))) => outcome,
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

/// A bounded reply is accepted only after its interpreter exits successfully.
async fn exchange_result(
    child: &mut Child,
    request: Vec<u8>,
) -> Result<(&mut Child, CustomSensorOutcome), ControlError> {
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| ControlError::Invalid("sensor input is unavailable".into()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ControlError::Invalid("sensor output is unavailable".into()))?;
    stdin.write_all(&request).await?;
    stdin.write_all(b"\n").await?;
    stdin.shutdown().await?;
    drop(stdin);
    let mut bytes: Vec<u8> = Vec::new();
    let mut stdout = stdout.take(SENSOR_RESULT_BYTES + 1);
    stdout.read_to_end(&mut bytes).await?;
    if bytes.len() as u64 > SENSOR_RESULT_BYTES || !child.wait().await?.success() {
        return Ok((child, CustomSensorOutcome::Failed));
    }
    Ok((
        child,
        serde_json::from_slice(&bytes).unwrap_or(CustomSensorOutcome::Failed),
    ))
}
