//! Command-output record validation, wire JSON and the integration's invocation context.

use crate::output_capture::main::invocation_context::invocation_context;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python, pyfunction};
use pyo3::types::{PyBytes, PyDict, PyDictMethods, PyString, PyStringMethods};
use sqlbuild_core::command_output::constants::{
    COMMAND_OUTPUT_LOSS_RECORD_TYPE, COMMAND_OUTPUT_RECORD_TYPE, CURRENT_SCHEMA_VERSION,
    INVOCATION_CONTEXT_ENV, MAX_INVOCATION_CONTEXT_DEPTH,
};
use sqlbuild_core::command_output::main::validate_output_record::validate_output_record;
use sqlbuild_core::command_output::models::RecordShape;
use sqlbuild_core::json::main::dumps::dumps;
use sqlbuild_core::json::models::{JsonDialect, JsonValue, StdlibJsonOptions};
use sqlbuild_host::entries::models::ExecutorHost;

use crate::output_capture::_helpers::values::freeze;
use crate::output_capture::_helpers::values::json_value;
use crate::output_capture::_helpers::values::object;
use crate::output_capture::_helpers::values::output_error;
use crate::output_capture::_helpers::values::plain_int;
use crate::output_capture::_helpers::values::validation_error;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const ENVELOPE_FIELDS: [&str; 16] = [
    "record_id",
    "record_type",
    "schema_version",
    "producer",
    "producer_version",
    "occurred_at",
    "invocation_id",
    "run_id",
    "sequence",
    "stream",
    "message",
    "chunk_index",
    "chunk_count",
    "priority",
    "dropped_records",
    "external_context",
];
const RECORD_FIELDS: [&str; 15] = [
    "invocation_id",
    "run_id",
    "sequence",
    "occurred_at",
    "stream",
    "message",
    "external_context",
    "chunk_index",
    "chunk_count",
    "priority",
    "record_type",
    "dropped_records",
    "schema_version",
    "producer",
    "producer_version",
];

fn optional_text(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    match value.downcast::<PyString>() {
        Ok(text) => Ok(Some(text.to_string_lossy().into_owned())),
        Err(_) => Ok(None),
    }
}

fn utc_aware(value: &Bound<'_, PyAny>) -> PyResult<bool> {
    let py = value.py();
    if !value.is_instance(&imported(py, "datetime", "datetime")?)?
        || native_field(value, "tzinfo")?.is_none()
    {
        return Ok(false);
    }
    let zero = imported(py, "datetime", "timedelta")?.call0()?;
    value.call_method0("utcoffset")?.eq(zero)
}

fn record_shape(record: &Bound<'_, PyAny>) -> PyResult<RecordShape> {
    let py = record.py();
    let field = |name: &str| record.getattr(name);
    let run_id = field("run_id")?;
    Ok(RecordShape {
        identity: [
            ("invocation_id", optional_text(&field("invocation_id")?)?),
            ("producer", optional_text(&field("producer")?)?),
            (
                "producer_version",
                optional_text(&field("producer_version")?)?,
            ),
        ],
        run_id: if run_id.is_none() {
            None
        } else {
            Some(optional_text(&run_id)?)
        },
        sequence: plain_int(&field("sequence")?),
        occurred_at_is_utc: utc_aware(&field("occurred_at")?)?,
        stream_is_known: field("stream")?
            .is_instance(&host_entry(py, ExecutorHost::CommandOutputStream)?)?,
        message_is_text: field("message")?.is_instance_of::<PyString>(),
        data_is_bytes: field("data")?.is_instance_of::<PyBytes>(),
        chunk_index: plain_int(&field("chunk_index")?),
        chunk_count: plain_int(&field("chunk_count")?),
        dropped_records: plain_int(&field("dropped_records")?),
        priority_is_known: field("priority")?
            .is_instance(&host_entry(py, ExecutorHost::OutputRecordPriority)?)?,
        record_type: optional_text(&field("record_type")?)?,
        schema_version: plain_int(&field("schema_version")?),
    })
}

/// The frozen `external_context` of a `CommandOutputRecord` that meets the wire contract.
#[pyfunction]
pub(crate) fn freeze_command_output_record<'py>(
    record: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = record.py();
    validate_output_record(&record_shape(record)?)
        .map_err(|message| validation_error(py, &message))?;
    let frozen = freeze(
        &native_field(record, "external_context")?,
        "external_context",
    )?;
    if !frozen.is_instance(&imported(py, "collections.abc", "Mapping")?)? {
        return Err(validation_error(
            py,
            "external_context must be a JSON object",
        ));
    }
    Ok(frozen)
}

/// Serialize one command-output record deterministically.
#[pyfunction]
pub(crate) fn command_output_to_json(record: &Bound<'_, PyAny>) -> PyResult<String> {
    let field = |name: &str| record.getattr(name);
    let occurred_at: String = call_with(
        &native_field(&field("occurred_at")?, "isoformat")?,
        &[("timespec", object(record.py(), "microseconds")?)],
    )?
    .extract()?;
    let mut entries: Vec<(String, JsonValue)> = Vec::new();
    for name in ENVELOPE_FIELDS {
        let value: JsonValue = match name {
            "occurred_at" => JsonValue::String(occurred_at.replace("+00:00", "Z")),
            "stream" | "priority" => json_value(&native_field(&field(name)?, "value")?)?,
            _ => json_value(&field(name)?)?,
        };
        entries.push((name.to_owned(), value));
    }
    let options = StdlibJsonOptions {
        sort_keys: true,
        allow_nan: false,
        ..StdlibJsonOptions::new(None).with_separators(",", ":")
    };
    dumps(&JsonValue::Object(entries), &JsonDialect::Stdlib(options))
        .map_err(|_| PyValueError::new_err("Out of range float values are not JSON compliant"))
}

fn occurred_at<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py = value.py();
    let message = "occurred_at must be an ISO-8601 string";
    let Ok(text) = value.downcast::<PyString>() else {
        return Err(validation_error(py, message));
    };
    let normalized = text.to_cow()?.replace('Z', "+00:00");
    imported(py, "datetime", "datetime")?
        .call_method1("fromisoformat", (normalized,))
        .map_err(|error| match error.is_instance_of::<PyValueError>(py) {
            true => validation_error(py, message),
            false => error,
        })
}

fn record_from_fields<'py>(data: &Bound<'py, PyDict>) -> PyResult<Bound<'py, PyAny>> {
    let py = data.py();
    let value = |name: &str| -> PyResult<Bound<'py, PyAny>> {
        data.get_item(name)?
            .ok_or_else(|| PyValueError::new_err(name.to_owned()))
    };
    let mut fields: Vec<(&str, Bound<'py, PyAny>)> = Vec::new();
    for name in RECORD_FIELDS {
        let converted = match name {
            "occurred_at" => occurred_at(&value(name)?)?,
            "stream" => {
                host_entry(py, ExecutorHost::CommandOutputStream)?.call1((value(name)?,))?
            }
            "priority" => {
                host_entry(py, ExecutorHost::OutputRecordPriority)?.call1((value(name)?,))?
            }
            _ => value(name)?,
        };
        fields.push((name, converted));
    }
    call_with(&host_entry(py, ExecutorHost::CommandOutputRecord)?, &fields)
}

/// Deserialize one known command-output record strictly.
#[pyfunction]
pub(crate) fn command_output_from_json<'py>(
    py: Python<'py>,
    raw_json: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let raw = imported(py, "json", "loads")?
        .call1((raw_json,))
        .map_err(|error| {
            match error.is_instance_of::<PyTypeError>(py)
                || error.is_instance_of::<PyValueError>(py)
            {
                true => validation_error(py, "command output must be valid JSON"),
                false => error,
            }
        })?;
    let Ok(data) = raw.downcast::<PyDict>() else {
        return Err(validation_error(py, "command output must be a JSON object"));
    };
    let mut keys: Vec<String> = Vec::new();
    for key in data.keys() {
        keys.push(key.extract()?);
    }
    keys.sort();
    let mut expected: Vec<&str> = ENVELOPE_FIELDS.to_vec();
    expected.sort_unstable();
    if keys != expected {
        return Err(validation_error(
            py,
            "command output fields do not match schema version 1",
        ));
    }
    let field = |name: &str| -> PyResult<Bound<'py, PyAny>> {
        data.get_item(name)?
            .ok_or_else(|| PyValueError::new_err(name.to_owned()))
    };
    if plain_int(&field("schema_version")?) != Some(CURRENT_SCHEMA_VERSION) {
        return Err(validation_error(
            py,
            "unsupported command output schema version",
        ));
    }
    let record_type = optional_text(&field("record_type")?)?;
    if !matches!(
        record_type.as_deref(),
        Some(COMMAND_OUTPUT_RECORD_TYPE | COMMAND_OUTPUT_LOSS_RECORD_TYPE)
    ) {
        return Err(validation_error(py, "unknown command output record type"));
    }
    if !field("external_context")?.is_instance_of::<PyDict>() {
        return Err(validation_error(
            py,
            "command output external_context must be an object",
        ));
    }
    let validation_class = host_entry(py, ExecutorHost::CommandOutputValidationError)?;
    let record = record_from_fields(data).map_err(|error| {
        let typed =
            error.is_instance_of::<PyTypeError>(py) || error.is_instance_of::<PyValueError>(py);
        if error.is_instance(py, &validation_class) || !typed {
            error
        } else {
            validation_error(py, "command output contains invalid field values")
        }
    })?;
    if !field("record_id")?.eq(native_field(&record, "record_id")?)? {
        return Err(validation_error(
            py,
            "command output record_id does not match its identity",
        ));
    }
    Ok(record)
}

pub(crate) fn validate_depth(value: &Bound<'_, PyAny>, depth: usize) -> Result<(), PyErr> {
    if depth > MAX_INVOCATION_CONTEXT_DEPTH {
        return Err(output_error(
            value.py(),
            ExecutorHost::OutputCaptureInputError,
            &format!(
                "{INVOCATION_CONTEXT_ENV} must not exceed {MAX_INVOCATION_CONTEXT_DEPTH} levels"
            ),
        ));
    }
    if let Ok(mapping) = value.downcast::<PyDict>() {
        for item in mapping.values() {
            validate_depth(&item, depth + 1)?;
        }
    } else if value.is_instance_of::<pyo3::types::PyList>() {
        for item in value.try_iter()? {
            validate_depth(&item?, depth + 1)?;
        }
    }
    Ok(())
}

pub(crate) fn input_error(py: Python<'_>, message: &str) -> PyErr {
    output_error(py, ExecutorHost::OutputCaptureInputError, message)
}

/// Return validated opaque invocation metadata supplied by an integration.
#[pyfunction]
#[pyo3(signature = (*, environment = None))]
pub(crate) fn invocation_context_from_environment<'py>(
    py: Python<'py>,
    environment: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    invocation_context(py, environment)
}
