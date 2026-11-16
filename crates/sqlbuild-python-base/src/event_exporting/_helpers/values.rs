//! Event exporter errors, severities, reprs and the lifecycle export policy catalogue across the Python boundary.

use crate::native_host::main::host_error::host_error;
use std::collections::HashMap;

use pyo3::exceptions::{PyKeyError, PyTypeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple, PyTupleMethods};
use sqlbuild_core::event_export::main::lifecycle_export_policy::lifecycle_export_policy;
use sqlbuild_core::event_export::main::sanitized_exception_type::sanitized_exception_type;
use sqlbuild_core::event_export::models::{ExportPolicy, ExportSeverity, ExporterCounts};
use sqlbuild_core::lifecycle_events::constants::CURRENT_LIFECYCLE_EVENT_SCHEMA_VERSION;
use sqlbuild_core::lifecycle_events::main::catalog::catalog;
use sqlbuild_host::entries::models::UserCodeHost;

use crate::records::main::construct::enum_from_value::enum_from_value;
use crate::records::types::RecordId;

/// `BoundEventExporter` parameters, in positional order.
const BOUND_EXPORTER_PARAMETERS: [&str; 5] = [
    "name",
    "function",
    "provider_arguments",
    "event_kinds",
    "min_severity",
];
const REQUIRED_BOUND_EXPORTER_PARAMETERS: usize = 3;

static POLICIES: PyOnceLock<HashMap<String, ExportPolicy>> = PyOnceLock::new();

/// The exception class `name` of the event exporter exceptions module, raised with `message`.
pub(crate) fn exporter_error(py: Python<'_>, name: UserCodeHost, message: &str) -> PyErr {
    host_error(py, name, message)
}

/// `EventExporterStateError(message)`.
pub(crate) fn state_error(py: Python<'_>, message: &str) -> PyErr {
    exporter_error(py, UserCodeHost::EventExporterStateError, message)
}

/// `EventExporterInputError(message)`.
pub(crate) fn input_error(py: Python<'_>, message: &str) -> PyErr {
    exporter_error(py, UserCodeHost::EventExporterInputError, message)
}

/// The Python `EventExportSeverity` member of `severity`.
pub(crate) fn severity_object(
    py: Python<'_>,
    severity: ExportSeverity,
) -> PyResult<Bound<'_, PyAny>> {
    enum_from_value(py, RecordId::EventExportSeverity, severity.value())
}

/// An `EventExportSeverity` or its string value, as `EventExportSeverity(value)` accepts it.
pub(crate) fn severity_from(value: &Bound<'_, PyAny>) -> PyResult<ExportSeverity> {
    let text: String = value.extract()?;
    ExportSeverity::from_value(&text).ok_or_else(|| {
        PyValueError::new_err(format!("'{text}' is not a valid EventExportSeverity"))
    })
}

/// The sanitized class name of a raised Python exception.
pub(crate) fn error_type_of(py: Python<'_>, error: &PyErr) -> String {
    let name: String = error
        .get_type(py)
        .getattr("__name__")
        .and_then(|name| name.extract())
        .unwrap_or_default();
    sanitized_exception_type(&name)
}

/// Export dimensions of every catalogued lifecycle event type, derived once per interpreter.
pub(crate) fn export_policies(py: Python<'_>) -> PyResult<&HashMap<String, ExportPolicy>> {
    POLICIES.get_or_try_init(py, || {
        let mut policies: HashMap<String, ExportPolicy> = HashMap::new();
        for definition in catalog(CURRENT_LIFECYCLE_EVENT_SCHEMA_VERSION).unwrap_or_default() {
            let policy: ExportPolicy =
                lifecycle_export_policy(definition.event_type, definition.terminal);
            policies.insert(definition.event_type.to_owned(), policy);
        }
        Ok::<_, pyo3::PyErr>(policies)
    })
}

/// The export policy of a validated event's type; `KeyError` for an uncatalogued type.
pub(crate) fn export_policy(py: Python<'_>, event_type: &str) -> PyResult<ExportPolicy> {
    export_policies(py)?
        .get(event_type)
        .cloned()
        .ok_or_else(|| PyKeyError::new_err(event_type.to_owned()))
}

/// `repr(value)`.
pub(crate) fn python_repr(value: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(value.repr()?.to_string())
}

/// `repr(text)`.
pub(crate) fn text_repr(py: Python<'_>, text: &str) -> PyResult<String> {
    python_repr(&PyString::new(py, text).into_any())
}

/// The dataclass-style repr of exporter counts.
pub(crate) fn counts_repr(counts: &ExporterCounts) -> String {
    format!(
        "EventExporterCounts(accepted={}, filtered={}, delivered={}, failed={}, dropped={})",
        counts.accepted, counts.filtered, counts.delivered, counts.failed, counts.dropped
    )
}

/// The dataclass-style repr of one exporter's accounting.
pub(crate) fn accounting_repr(
    py: Python<'_>,
    (name, counts): (&str, &ExporterCounts),
) -> PyResult<String> {
    Ok(format!(
        "EventExporterAccounting(exporter_name={}, counts={})",
        text_repr(py, name)?,
        counts_repr(counts)
    ))
}

/// `BoundEventExporter(...)` arguments by parameter, as Python binds a dataclass signature.
pub(crate) fn bound_exporter_arguments<'py>(
    arguments: &Bound<'py, PyTuple>,
    options: Option<&Bound<'py, PyDict>>,
) -> PyResult<[Option<Bound<'py, PyAny>>; 5]> {
    let mut bound: [Option<Bound<'py, PyAny>>; 5] = Default::default();
    if arguments.len() > BOUND_EXPORTER_PARAMETERS.len() {
        return Err(PyTypeError::new_err(format!(
            "BoundEventExporter() takes at most {} positional arguments",
            BOUND_EXPORTER_PARAMETERS.len()
        )));
    }
    for (slot, argument) in bound.iter_mut().zip(arguments.iter()) {
        *slot = Some(argument);
    }
    for (name, value) in options.into_iter().flat_map(|options| options.iter()) {
        let name: String = name.extract()?;
        let Some(position) = BOUND_EXPORTER_PARAMETERS
            .iter()
            .position(|parameter| *parameter == name)
        else {
            return Err(PyTypeError::new_err(format!(
                "BoundEventExporter() got an unexpected keyword argument '{name}'"
            )));
        };
        if bound[position].replace(value).is_some() {
            return Err(PyTypeError::new_err(format!(
                "BoundEventExporter() got multiple values for argument '{name}'"
            )));
        }
    }
    let missing: Option<&&str> = BOUND_EXPORTER_PARAMETERS
        .iter()
        .take(REQUIRED_BOUND_EXPORTER_PARAMETERS)
        .zip(&bound)
        .find(|(_, value)| value.is_none())
        .map(|(name, _)| name);
    if let Some(name) = missing {
        return Err(PyTypeError::new_err(format!(
            "BoundEventExporter() missing required argument: '{name}'"
        )));
    }
    Ok(bound)
}
