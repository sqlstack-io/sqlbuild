//! Node error text is normalized while a run change is read, so no caller stores or returns it raw.

use serde::{Deserialize, Deserializer};
use serde_json::Value;

use sqlbuild_core::redaction::main::redact_error_text::redact_error_text;

use crate::runs::models::{RunChange, RunChangeRequest};
use crate::store::errors::ControlError;

/// The longest failure phase token a worker may report.
const MAX_PHASE_LENGTH: usize = 32;

pub(crate) fn bounded_error_text<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.map(|text| redact_error_text(&text)))
}

/// A failure phase as a worker reports it: kept only as a short lowercase token, so it can't carry text.
pub(crate) fn failure_phase<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.filter(|phase| is_phase_token(phase)))
}

fn is_phase_token(phase: &str) -> bool {
    (1..=MAX_PHASE_LENGTH).contains(&phase.len()) && phase.bytes().all(is_phase_byte)
}

const fn is_phase_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte == b'_'
}

/// The error text and failure phase of a stored node-finished fact; both `None` when there is no fact.
pub(crate) fn finished_failure(
    payload: &Value,
) -> Result<(Option<String>, Option<String>), ControlError> {
    if payload.is_null() {
        return Ok((None, None));
    }
    Ok(
        match serde_json::from_value::<RunChangeRequest>(payload["change"].clone())
            .map_err(|error| ControlError::Invalid(error.to_string()))?
            .change
        {
            RunChange::NodeFinished {
                error_message,
                phase,
                ..
            } => (error_message, phase),
            _ => (None, None),
        },
    )
}
