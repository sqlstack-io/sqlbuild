//! The envelope and promoted payload columns of a lifecycle event row.

use serde_json::{Map, Value};
use sqlbuild_core::event_export::main::lifecycle_export_policy::lifecycle_export_policy;
use sqlbuild_core::lifecycle_events::main::catalog::catalog;

use crate::records::_helpers::rows::{optional_text, text};
use crate::records::constants::{
    CORRELATION_FIELDS, PROMOTED_INTEGER_FIELDS, PROMOTED_REAL_FIELDS, PROMOTED_TEXT_FIELDS,
    STARTED_AT_FIELD,
};
use crate::records::errors::RecordError;
use crate::records::models::RecallValue;

/// One named column value.
pub(crate) type Column = (&'static str, RecallValue);

fn required_text<'a>(event: &'a Map<String, Value>, field: &str) -> Result<&'a str, RecordError> {
    event
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| RecordError::new(format!("lifecycle event {field} must be a string")))
}

fn is_terminal(event_type: &str, schema_version: i64) -> bool {
    catalog(schema_version)
        .into_iter()
        .flatten()
        .any(|definition| definition.event_type == event_type && definition.terminal)
}

/// The event JSON object.
pub(crate) fn event_object(event_json: &str) -> Result<Map<String, Value>, RecordError> {
    match serde_json::from_str(event_json) {
        Ok(Value::Object(event)) => Ok(event),
        Ok(_) => Err(RecordError::new("lifecycle event must be a JSON object")),
        Err(error) => Err(RecordError::new(format!(
            "lifecycle event is not JSON: {error}"
        ))),
    }
}

/// The export severity of an event's type; `None` without a text `event_type`.
pub(crate) fn severity_of(event: &Map<String, Value>) -> Option<&'static str> {
    let event_type: &str = event.get("event_type").and_then(Value::as_str)?;
    let schema_version: i64 = event
        .get("schema_version")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    Some(
        lifecycle_export_policy(event_type, is_terminal(event_type, schema_version))
            .severity
            .value(),
    )
}

/// Identity, type, export policy and correlation columns.
pub(crate) fn envelope_columns(event: &Map<String, Value>) -> Result<Vec<Column>, RecordError> {
    let event_type: &str = required_text(event, "event_type")?;
    let schema_version: i64 = event
        .get("schema_version")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let policy = lifecycle_export_policy(event_type, is_terminal(event_type, schema_version));
    let mut columns: Vec<Column> = vec![
        ("event_id", text(required_text(event, "event_id")?)),
        (
            "invocation_sequence",
            event
                .get("invocation_sequence")
                .and_then(Value::as_i64)
                .map_or(RecallValue::Null, RecallValue::Integer),
        ),
        ("occurred_at", text(required_text(event, "occurred_at")?)),
        ("event_type", text(event_type)),
        ("event_kind", text(policy.kind)),
        ("severity", text(policy.severity.value())),
        ("producer", text(required_text(event, "producer")?)),
        ("event_schema_version", RecallValue::Integer(schema_version)),
        (
            STARTED_AT_FIELD,
            optional_text(event.get(STARTED_AT_FIELD).and_then(Value::as_str)),
        ),
    ];
    columns.extend(CORRELATION_FIELDS.iter().map(|field| {
        (
            *field,
            optional_text(event.get(*field).and_then(Value::as_str)),
        )
    }));
    Ok(columns)
}

/// Payload fields promoted to columns, then the payload itself.
pub(crate) fn payload_columns(payload: &Map<String, Value>) -> Vec<Column> {
    let mut columns: Vec<Column> = PROMOTED_TEXT_FIELDS
        .iter()
        .map(|field| {
            (
                *field,
                optional_text(payload.get(*field).and_then(Value::as_str)),
            )
        })
        .collect();
    columns.extend(PROMOTED_INTEGER_FIELDS.iter().map(|field| {
        let value = payload.get(*field).and_then(Value::as_i64);
        (
            *field,
            value.map_or(RecallValue::Null, RecallValue::Integer),
        )
    }));
    columns.extend(PROMOTED_REAL_FIELDS.iter().map(|field| {
        let value = payload.get(*field).and_then(Value::as_f64);
        (*field, value.map_or(RecallValue::Null, RecallValue::Real))
    }));
    columns.push(("payload", text(Value::Object(payload.clone()).to_string())));
    columns
}
