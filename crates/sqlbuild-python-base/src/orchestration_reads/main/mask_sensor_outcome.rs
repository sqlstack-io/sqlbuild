//! A tick's text with the providers' secret values masked and credential-shaped text redacted, before the location returns it.

use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_core::redaction::main::redact_masked_text::redact_masked_text;
use sqlbuild_orchestration::sensors::models::SensorFunctionOutcome;

use crate::orchestration_reads::_helpers::instance_secrets::instance_secret_values;
use crate::orchestration_reads::_helpers::secret_forms::secret_forms;
use crate::orchestration_reads::constants::SECRET_MASK;

/// `outcome` with each of `secrets`, and each secret setting of the provider `instances`, as it is, percent-encoded or JSON-escaped, replaced by `****` in its traceback or skip reason, then the usual pattern redaction.
pub fn mask_sensor_outcome(
    outcome: SensorFunctionOutcome,
    (secrets, instances): (&[String], &[Bound<'_, PyAny>]),
) -> PyResult<SensorFunctionOutcome> {
    let mut forms: Vec<String> = secrets
        .iter()
        .cloned()
        .chain(instance_secret_values(instances)?)
        .flat_map(|secret| secret_forms(&secret))
        .collect();
    forms.sort_by(|left, right| right.len().cmp(&left.len()).then_with(|| left.cmp(right)));
    forms.dedup();
    let mask = |text: String| {
        redact_masked_text(&text, |reading| {
            forms.iter().fold(reading.to_owned(), |text, form| {
                text.replace(form.as_str(), SECRET_MASK)
            })
        })
    };
    Ok(match outcome {
        SensorFunctionOutcome::Failed { error } => {
            SensorFunctionOutcome::Failed { error: mask(error) }
        }
        SensorFunctionOutcome::Skipped { cursor, reason } => SensorFunctionOutcome::Skipped {
            cursor,
            reason: mask(reason),
        },
        outcome => outcome,
    })
}
