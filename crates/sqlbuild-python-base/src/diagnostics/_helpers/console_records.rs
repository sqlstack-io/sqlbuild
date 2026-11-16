//! The `--debug` stderr line of one Python log record.

use crate::records::main::fields::field::field as native_field;
use pyo3::exceptions::PyOverflowError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult};
use pyo3::types::{PyInt, PyString};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::diagnostic_routing::constants::SQL_TEXT_FIELD;
use sqlbuild_core::diagnostic_text::main::format_diagnostic_record::format_diagnostic_record;
use sqlbuild_core::diagnostic_text::models::DiagnosticRecordText;

/// The record field holding an operation's duration in milliseconds.
const DURATION_FIELD: &str = "sqlbuild_duration_ms";
const MILLISECONDS_PER_SECOND: i64 = 1000;

/// Each context field shown as a `label=value` token, in display order.
const CONTEXT_TOKENS: [(&str, &str); 7] = [
    ("sqlbuild_phase", "phase"),
    ("sqlbuild_kind", "kind"),
    ("sqlbuild_action_name", "action"),
    ("sqlbuild_audit_name", "audit"),
    ("sqlbuild_column_name", "column"),
    ("sqlbuild_window", "window"),
    ("sqlbuild_status", "status"),
];

/// `getattr(record, field, None)` when it is a `str`.
fn text_field(record: &Bound<'_, PyAny>, field: &str) -> PyResult<Option<String>> {
    if !record.hasattr(field)? {
        return Ok(None);
    }
    let value: Bound<'_, PyAny> = native_field(record, field)?;
    if value.is_instance_of::<PyString>() {
        return Ok(Some(value.extract()?));
    }
    Ok(None)
}

/// Python's `sqlbuild_duration_ms / 1000` for an integer field that fits a float, else `None`.
fn duration_seconds(record: &Bound<'_, PyAny>) -> PyResult<Option<f64>> {
    if !record.hasattr(DURATION_FIELD)? {
        return Ok(None);
    }
    let value: Bound<'_, PyAny> = native_field(record, DURATION_FIELD)?;
    if !value.is_instance_of::<PyInt>() {
        return Ok(None);
    }
    match value.div(MILLISECONDS_PER_SECOND) {
        Ok(seconds) => Ok(Some(seconds.extract()?)),
        Err(error) if error.is_instance_of::<PyOverflowError>(record.py()) => Ok(None),
        Err(error) => Err(error),
    }
}

/// The console line of `record` without its exception text.
#[pyfunction]
#[pyo3(signature = (record, *, use_color))]
fn format_diagnostics_console_record(
    record: &Bound<'_, PyAny>,
    use_color: bool,
) -> PyResult<String> {
    let mut tokens: Vec<(&'static str, String)> = Vec::new();
    for (field, label) in CONTEXT_TOKENS {
        if let Some(value) = text_field(record, field)? {
            tokens.push((label, value));
        }
    }
    let subject: Option<String> = text_field(record, "sqlbuild_subject")?;
    let duration_seconds: Option<f64> = if subject.is_some() {
        duration_seconds(record)?
    } else {
        None
    };
    let text = DiagnosticRecordText {
        level_name: native_field(record, "levelname")?.extract()?,
        logger_name: native_field(record, "name")?.extract()?,
        message: record.call_method0("getMessage")?.extract()?,
        subject,
        name: text_field(record, "sqlbuild_name")?,
        event: text_field(record, "sqlbuild_event")?,
        tokens,
        duration_seconds,
        sql: text_field(record, SQL_TEXT_FIELD)?,
    };
    Ok(format_diagnostic_record(&text, CliStyle { use_color }))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(format_diagnostics_console_record, module)?)
}
