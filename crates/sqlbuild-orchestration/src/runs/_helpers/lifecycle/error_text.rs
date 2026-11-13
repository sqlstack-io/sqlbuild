//! Node error text is normalized while a run change is read, so no caller stores or returns it raw.

use serde::{Deserialize, Deserializer};
use serde_json::Value;

use sqlbuild_core::redaction::main::redact_error_text::redact_error_text;

use crate::runs::models::{RunChange, RunChangeRequest};
use crate::store::errors::ControlError;

pub(crate) fn bounded_error_text<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.map(|text| redact_error_text(&text)))
}

/// The error text of a stored node-finished fact; `None` when there is no fact or it carries none.
pub(crate) fn finished_error_message(payload: &Value) -> Result<Option<String>, ControlError> {
    if payload.is_null() {
        return Ok(None);
    }
    Ok(
        match serde_json::from_value::<RunChangeRequest>(payload["change"].clone())
            .map_err(|error| ControlError::Invalid(error.to_string()))?
            .change
        {
            RunChange::NodeFinished { error_message, .. } => error_message,
            _ => None,
        },
    )
}
