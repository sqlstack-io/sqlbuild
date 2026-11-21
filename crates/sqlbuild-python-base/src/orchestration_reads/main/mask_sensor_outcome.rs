//! A tick's text with the providers' secret values masked and credential-shaped text redacted, before the location returns it.

use sqlbuild_core::redaction::main::redact_text::redact_text;
use sqlbuild_orchestration::sensors::models::SensorFunctionOutcome;

use crate::orchestration_reads::constants::SECRET_MASK;

/// `outcome` with each of `secrets` replaced by `****` in its traceback or skip reason, then the usual pattern redaction.
#[must_use]
pub fn mask_sensor_outcome(
    outcome: SensorFunctionOutcome,
    secrets: &[String],
) -> SensorFunctionOutcome {
    let mask = |text: String| {
        redact_text(&secrets.iter().fold(text, |text, secret| {
            text.replace(secret.as_str(), SECRET_MASK)
        }))
    };
    match outcome {
        SensorFunctionOutcome::Failed { error } => {
            SensorFunctionOutcome::Failed { error: mask(error) }
        }
        SensorFunctionOutcome::Skipped { cursor, reason } => SensorFunctionOutcome::Skipped {
            cursor,
            reason: mask(reason),
        },
        outcome => outcome,
    }
}
