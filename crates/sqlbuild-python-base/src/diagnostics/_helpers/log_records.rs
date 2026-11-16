//! Structured diagnostic record fields, destination routing and compute log retention.

use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{
    PyBool, PyDict, PyDictMethods, PyFloat, PyInt, PyString, PyStringMethods, PyTuple,
};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::diagnostic_routing::constants::{
    DIAGNOSTIC_PRODUCER, DIAGNOSTIC_SOURCE, FIELD_PREFIX, FORMATTER_RECORD_FIELDS, IDENTITY_FIELDS,
    INVOCATION_ID_FIELD, LOGGER_PREFIX, LOGGER_ROOT_NAME, REDACTED_SQL_LOG_MESSAGE, SAFE_FIELDS,
    SQL_ACTION_FIELD, SQL_DIGEST_FIELD, SQL_INTENT_FIELD, SQL_LOGGER_NAME,
    SQL_PRIVATE_RECORD_FIELD, SQL_QUERY_ID_FIELD, SQL_SAFE_FIELDS, SQL_TEXT_DIAGNOSTIC_FIELD,
    SQL_TEXT_FIELD, UNFORMATTABLE_MESSAGE,
};
use sqlbuild_core::diagnostic_routing::main::compute_log_admits::compute_log_admits;
use sqlbuild_core::diagnostic_routing::main::console_summary_line::console_summary_line;
use sqlbuild_core::diagnostic_routing::main::diagnostic_severity::diagnostic_severity;
use sqlbuild_core::diagnostic_routing::main::internal_logger::internal_logger;
use sqlbuild_core::diagnostic_routing::main::project_source_path::project_source_path;
use sqlbuild_core::diagnostic_routing::main::route_admits::route_admits;
use sqlbuild_core::diagnostic_routing::main::sql_digest::sql_digest;
use sqlbuild_core::diagnostic_routing::main::third_party_console_form::third_party_console_form;
use sqlbuild_core::diagnostic_routing::models::{RecordFacts, RouteOptions, ThirdPartyConsoleForm};
use sqlbuild_core::redaction::main::redact_text::redact_text;
use sqlbuild_host::entries::models::ExecutorHost;

use crate::boundary::main::imported::imported;
use crate::boundary::models::Request;
use crate::diagnostics::_helpers::redaction::redacted_value;
use crate::native_host::main::host_entry::host_entry;
use crate::records::main::fields::field::field as native_field;
use crate::records::main::fields::mapping_items::mapping_items;

const PRIVATE_SQL_MARKER: ExecutorHost = ExecutorHost::SqlPrivateRecordMarker;

/// `context | fields | identity`, with later mappings overriding earlier keys in place.
fn merged<'py>(
    py: Python<'py>,
    parts: &[Option<&Bound<'py, PyAny>>],
    skip: Option<&str>,
) -> PyResult<Bound<'py, PyDict>> {
    let merged: Bound<'py, PyDict> = PyDict::new(py);
    for part in parts.iter().flatten() {
        if part.is_none() {
            continue;
        }
        for item in mapping_items(part)?.try_iter()? {
            let (key, value): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
            if skip.is_some_and(|skipped| key.eq(skipped).unwrap_or(false)) {
                continue;
            }
            merged.set_item(key, value)?;
        }
    }
    Ok(merged)
}

/// The record fields of the current execution identity, each named `sqlbuild_<field>`.
fn identity_fields<'py>(
    py: Python<'py>,
    identity: Option<&Bound<'py, PyAny>>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let Some(identity) = identity.filter(|identity| !identity.is_none()) else {
        return Ok(None);
    };
    let fields: Bound<'py, PyDict> = PyDict::new(py);
    for item in mapping_items(identity)?.try_iter()? {
        let (key, value): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
        fields.set_item(format!("{FIELD_PREFIX}{}", key.str()?.to_str()?), value)?;
    }
    Ok(Some(fields.into_any()))
}

/// The message and record fields of one logged SQL statement, which never carry its text.
#[pyfunction]
#[pyo3(signature = (**request))]
fn sql_log_record<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<(String, Bound<'py, PyDict>)> {
    let request: Request<'_, 'py> = Request::new("sql_log_record", request)?;
    let py: Python<'py> = request.py();
    let sql: String = request.get("sql")?.extract()?;
    let action: Bound<'py, PyAny> = request.get("action")?;
    let sql_fields: Bound<'py, PyDict> = PyDict::new(py);
    sql_fields.set_item(SQL_ACTION_FIELD, &action)?;
    sql_fields.set_item(SQL_DIGEST_FIELD, sql_digest(&sql))?;
    for (field, name) in [
        (SQL_INTENT_FIELD, "intent"),
        (SQL_QUERY_ID_FIELD, "query_id"),
    ] {
        let value: Bound<'py, PyAny> = request.get(name)?;
        if !value.is_none() {
            sql_fields.set_item(field, value)?;
        }
    }
    let identity: Option<Bound<'py, PyAny>> = identity_fields(py, Some(&request.get("identity")?))?;
    let fields: Bound<'py, PyDict> = merged(
        py,
        &[
            Some(&request.get("context")?),
            Some(sql_fields.as_any()),
            identity.as_ref(),
        ],
        Some(SQL_TEXT_FIELD),
    )?;
    let message: String = format!("{} SQL", action.str()?.to_str()?);
    Ok((message, fields))
}

/// The record fields of one structured debug event.
#[pyfunction]
#[pyo3(signature = (*, context, event_fields, identity))]
fn debug_event_record_fields<'py>(
    py: Python<'py>,
    context: &Bound<'py, PyAny>,
    event_fields: &Bound<'py, PyAny>,
    identity: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let identity: Option<Bound<'py, PyAny>> = identity_fields(py, Some(identity))?;
    merged(
        py,
        &[Some(context), Some(event_fields), identity.as_ref()],
        None,
    )
}

/// Whether a logger name belongs to SQLBuild, with Python semantics for non-string names.
fn internal_name(name: &Bound<'_, PyAny>) -> PyResult<bool> {
    if let Ok(text) = name.downcast::<PyString>() {
        return Ok(internal_logger(text.to_str()?));
    }
    Ok(name.eq(LOGGER_ROOT_NAME)?
        || name
            .call_method1("startswith", (LOGGER_PREFIX,))?
            .is_truthy()?)
}

/// `record.__dict__`.
fn record_values<'py>(record: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    Ok(record.getattr("__dict__")?.downcast_into::<PyDict>()?)
}

/// `Ok(None)` for an `Exception`, which the caller swallows; other errors propagate.
fn swallowed<'py, T>(py: Python<'py>, result: PyResult<T>) -> PyResult<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.is_instance_of::<PyException>(py) => Ok(None),
        Err(error) => Err(error),
    }
}

/// The redacted formatted message of a record.
fn redacted_message(record: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(redact_text(
        record.call_method0("getMessage")?.extract::<&str>()?,
    ))
}

/// The redacted formatted traceback of a record's `exc_info`, if it has one that formats.
fn exception_text(record: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    let py: Python<'_> = record.py();
    let exc_info: Bound<'_, PyAny> = native_field(record, "exc_info")?;
    if exc_info.is_none() {
        return Ok(None);
    }
    let formatted: PyResult<String> = imported(py, "logging", "Formatter")
        .and_then(|formatter| formatter.call0())
        .and_then(|formatter| formatter.call_method1("formatException", (exc_info,)))
        .and_then(|text| text.extract::<String>())
        .map(|text| redact_text(&text));
    swallowed(py, formatted)
}

/// Whether the record's logging call is in a file under one of the project roots.
fn project_record(record: &Bound<'_, PyAny>, roots: Option<Bound<'_, PyAny>>) -> PyResult<bool> {
    let Some(roots) = roots.filter(|roots| !roots.is_none()) else {
        return Ok(false);
    };
    let Ok(pathname) = native_field(record, "pathname")?.extract::<String>() else {
        return Ok(false);
    };
    Ok(project_source_path(
        &pathname,
        &roots.extract::<Vec<String>>()?,
    ))
}

/// A redacted copy of `record.__dict__` for one destination, or `None` when it does not belong.
#[pyfunction]
#[pyo3(signature = (**request))]
fn route_diagnostic_log_record<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Option<Bound<'py, PyDict>>> {
    let request: Request<'_, 'py> = Request::new("route_diagnostic_log_record", request)?;
    let record: &Bound<'py, PyAny> = &request.get("record")?;
    let flag = |name: &str| -> PyResult<bool> { request.get(name)?.extract() };
    let options = RouteOptions {
        internal_only: flag("internal_only")?,
        allow_internal: flag("allow_internal")?,
        include_sql_text: flag("include_sql_text")?,
    };
    let summarize_third_party: bool = match request.optional("summarize_third_party")? {
        Some(value) => value.extract()?,
        None => false,
    };
    let py: Python<'py> = record.py();
    let values: Bound<'py, PyDict> = record_values(record)?;
    let facts = RecordFacts {
        internal: internal_name(&native_field(record, "name")?)?,
        level: native_field(record, "levelno")?.extract()?,
        has_sql_digest: values.contains(SQL_DIGEST_FIELD)?,
        has_sql_text: values.contains(SQL_TEXT_FIELD)?,
    };
    if !route_admits(options, facts) {
        return Ok(None);
    }
    let form: ThirdPartyConsoleForm = if summarize_third_party
        && !facts.internal
        && !project_record(record, request.optional("project_roots")?)?
    {
        third_party_console_form(facts.level)
    } else {
        ThirdPartyConsoleForm::Full
    };
    if form == ThirdPartyConsoleForm::Hidden {
        return Ok(None);
    }
    let copied: Bound<'py, PyDict> = values.copy()?;
    let mut message: String = swallowed(py, redacted_message(record))?
        .unwrap_or_else(|| UNFORMATTABLE_MESSAGE.to_owned());
    copied.set_item("args", PyTuple::empty(py))?;
    if facts.has_sql_digest {
        REDACTED_SQL_LOG_MESSAGE.clone_into(&mut message);
    }
    if form == ThirdPartyConsoleForm::Summary {
        message = console_summary_line(&message).to_owned();
        copied.set_item("stack_info", py.None())?;
    } else if let Some(exception_text) = exception_text(record)? {
        message = format!("{message}\n{exception_text}");
    }
    if !native_field(record, "exc_info")?.is_none() {
        copied.set_item("exc_info", py.None())?;
        copied.set_item("exc_text", py.None())?;
    }
    copied.set_item("msg", message)?;
    for (key, value) in copied.copy()?.iter() {
        let name: String = key.extract()?;
        if name.starts_with(FIELD_PREFIX) && name != SQL_TEXT_FIELD {
            copied.set_item(&name, redacted_value(&name, &value)?)?;
        }
    }
    if !options.include_sql_text && copied.contains(SQL_TEXT_FIELD)? {
        copied.del_item(SQL_TEXT_FIELD)?;
    }
    Ok(Some(copied))
}

/// Redacted structured extras of a record, excluding standard, formatter and SQL text fields.
fn extra_fields<'py>(record: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = record.py();
    let standard: Bound<'py, PyDict> =
        record_values(&imported(py, "logging", "makeLogRecord")?.call1((PyDict::new(py),))?)?;
    let extras: Bound<'py, PyDict> = PyDict::new(py);
    for (key, value) in record_values(record)?.iter() {
        let name: String = key.extract()?;
        let excluded: bool = standard.contains(&name)?
            || FORMATTER_RECORD_FIELDS.contains(&name.as_str())
            || name == SQL_TEXT_FIELD;
        if !excluded {
            extras.set_item(&name, redacted_value(&name, &value)?)?;
        }
    }
    Ok(extras)
}

/// Redacted non-standard fields of a record without evaluating arbitrary formatters.
#[pyfunction]
#[pyo3(signature = (record))]
fn safe_diagnostic_extra_fields<'py>(record: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    extra_fields(record)
}

/// Whether a redacted structured value is a JSON scalar.
fn scalar(value: &Bound<'_, PyAny>) -> bool {
    value.is_instance_of::<PyString>()
        || value.is_instance_of::<PyInt>()
        || value.is_instance_of::<PyFloat>()
        || value.is_instance_of::<PyBool>()
}

/// One record compute logs retain, with its safe structured fields.
struct RetainedRecord<'a, 'py> {
    record: &'a Bound<'py, PyAny>,
    values: &'a Bound<'py, PyDict>,
    facts: RecordFacts,
    include_sql_text: bool,
}

/// The message, logger and fields of a retained record, or an error the handler swallows.
fn retained<'py>(
    retained: &RetainedRecord<'_, 'py>,
    fields: Bound<'py, PyDict>,
) -> PyResult<(String, Bound<'py, PyAny>, Bound<'py, PyDict>)> {
    let RetainedRecord {
        record,
        values,
        facts,
        include_sql_text,
    } = *retained;
    let py: Python<'py> = record.py();
    let contains_sql: bool = facts.has_sql_digest || facts.has_sql_text;
    if !contains_sql {
        let mut message: String = redacted_message(record)?;
        let fields: Bound<'py, PyDict> = if facts.internal {
            fields
        } else {
            if let Some(exception_text) = exception_text(record)? {
                message = format!("{message}\n{exception_text}");
            }
            extra_fields(record)?
        };
        return Ok((message, native_field(record, "name")?, fields));
    }
    let sql_fields: Bound<'py, PyDict> = PyDict::new(py);
    for (key, value) in fields.iter() {
        if SQL_SAFE_FIELDS.contains(&key.extract::<&str>()?) {
            sql_fields.set_item(key, value)?;
        }
    }
    if include_sql_text && let Some(text) = values.get_item(SQL_TEXT_FIELD)? {
        sql_fields.set_item(SQL_TEXT_DIAGNOSTIC_FIELD, text.str()?)?;
    }
    Ok((
        REDACTED_SQL_LOG_MESSAGE.to_owned(),
        PyString::new(py, SQL_LOGGER_NAME).into_any(),
        sql_fields,
    ))
}

/// The `DiagnosticLog` fields compute logs retain for a record, or `None` when it is not kept.
#[pyfunction]
#[pyo3(signature = (*, record, invocation_id, include_sql_text))]
fn compute_diagnostic_log_fields<'py>(
    py: Python<'py>,
    record: &Bound<'py, PyAny>,
    invocation_id: &str,
    include_sql_text: bool,
) -> PyResult<Option<Bound<'py, PyDict>>> {
    let name: Bound<'py, PyAny> = native_field(record, "name")?;
    let Ok(name) = name.downcast::<PyString>() else {
        return Ok(None);
    };
    let values: Bound<'py, PyDict> = record_values(record)?;
    let level: i64 = native_field(record, "levelno")?.extract()?;
    let facts = RecordFacts {
        internal: internal_logger(name.to_str()?),
        level,
        has_sql_digest: values.contains(SQL_DIGEST_FIELD)?,
        has_sql_text: values.contains(SQL_TEXT_FIELD)?,
    };
    if !compute_log_admits(facts, include_sql_text) {
        return Ok(None);
    }
    let trusted_private_sql: bool = match values.get_item(SQL_PRIVATE_RECORD_FIELD)? {
        Some(marker) => marker.is(&host_entry(py, PRIVATE_SQL_MARKER)?),
        None => false,
    };
    let mut identities: Vec<(&str, Option<String>)> = Vec::new();
    for field in IDENTITY_FIELDS {
        let value: Option<String> = match values.get_item(format!("{FIELD_PREFIX}{field}"))? {
            Some(value) if !value.is_none() => Some(value.str()?.to_str()?.to_owned()),
            _ => None,
        };
        identities.push((field, value));
    }
    let fields: Bound<'py, PyDict> = PyDict::new(py);
    for safe_name in SAFE_FIELDS {
        let raw: Bound<'py, PyAny> = values
            .get_item(format!("{FIELD_PREFIX}{safe_name}"))?
            .unwrap_or_else(|| py.None().into_bound(py));
        let safe_value: Bound<'py, PyAny> = redacted_value(safe_name, &raw)?;
        if scalar(&safe_value) {
            fields.set_item(safe_name, safe_value)?;
        }
    }
    let record_to_retain = RetainedRecord {
        record,
        values: &values,
        facts,
        include_sql_text,
    };
    let Some((message, logger, fields)) = swallowed(py, retained(&record_to_retain, fields))?
    else {
        return Ok(None);
    };
    let contains_sql: bool = facts.has_sql_digest || facts.has_sql_text;
    let diagnostic: Bound<'py, PyDict> = PyDict::new(py);
    diagnostic.set_item("producer", DIAGNOSTIC_PRODUCER)?;
    diagnostic.set_item("severity", diagnostic_severity(level))?;
    diagnostic.set_item("logger", logger)?;
    diagnostic.set_item("source", DIAGNOSTIC_SOURCE)?;
    diagnostic.set_item("message", message)?;
    diagnostic.set_item("fields", fields)?;
    for (field, value) in identities {
        let value: Option<String> = if contains_sql && !trusted_private_sql {
            None
        } else {
            value
        };
        let value: Option<String> = if field == INVOCATION_ID_FIELD
            && (contains_sql || value.as_deref().is_none_or(str::is_empty))
        {
            Some(invocation_id.to_owned())
        } else {
            value
        };
        diagnostic.set_item(field, value)?;
    }
    Ok(Some(diagnostic))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(sql_log_record, module)?)?;
    module.add_function(wrap_pyfunction!(debug_event_record_fields, module)?)?;
    module.add_function(wrap_pyfunction!(route_diagnostic_log_record, module)?)?;
    module.add_function(wrap_pyfunction!(safe_diagnostic_extra_fields, module)?)?;
    module.add_function(wrap_pyfunction!(compute_diagnostic_log_fields, module)?)
}
