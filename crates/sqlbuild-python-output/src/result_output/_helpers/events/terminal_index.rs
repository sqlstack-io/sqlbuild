//! The invocation-scoped index of canonical lifecycle facts that result output projects from.

use crate::result_output::models::TerminalEventIndex;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyRef, PyResult, Python};
use pyo3::pyclass;
use pyo3::types::{PyInt, PyString};

use sqlbuild_core::terminal_events::models::{TerminalFact, TerminalIndex};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::main::registry::native_attr::native_attr;
use sqlbuild_python_base::records::types::NativeAttr;

/// The module attribute holding the `ContextVar` of the installed index.
pub(crate) const CONTEXT_ATTRIBUTE: &str = "_terminal_event_index_context";
pub(crate) const CONTEXT_NAME: &str = "sqlbuild_terminal_event_index";

#[derive(Debug)]
pub(crate) struct IndexState {
    pub(crate) index: TerminalIndex,
    pub(crate) events: Vec<Py<PyAny>>,
}

/// One claimed terminal with its immutable canonical publication sequence.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct TerminalEventClaim {
    #[pyo3(get)]
    pub(crate) terminal: Py<PyAny>,
    #[pyo3(get)]
    pub(crate) event_sequence: usize,
}

/// `payload.get(key)` when it is a string.
fn payload_text(payload: &Bound<'_, PyAny>, key: &str) -> PyResult<Option<String>> {
    let value: Bound<'_, PyAny> = mapping_get(payload, key)?;
    if value.is_instance_of::<PyString>() {
        return Ok(Some(value.extract()?));
    }
    Ok(None)
}

/// The payload attempt number, when it is an `int` (or `bool`, which Python compares as one).
fn payload_attempt_number(payload: &Bound<'_, PyAny>) -> PyResult<Option<i64>> {
    let value: Bound<'_, PyAny> = mapping_get(payload, "attempt_number")?;
    if !value.is_instance_of::<PyInt>() {
        return Ok(None);
    }
    let Ok(number) = value.extract::<i64>() else {
        return Ok(None);
    };
    Ok(Some(number))
}

fn optional_text(event: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<String>> {
    native_field(event, name)?.extract()
}

/// The facts of one lifecycle event that terminal indexing reads.
pub(crate) fn terminal_fact(event: &Bound<'_, PyAny>) -> PyResult<TerminalFact> {
    let payload: Bound<'_, PyAny> = native_field(event, "payload")?;
    Ok(TerminalFact {
        event_id: native_field(event, "event_id")?.extract()?,
        event_type: native_field(event, "event_type")?.extract()?,
        resource_id: optional_text(event, "resource_id")?,
        resource_name: payload_text(&payload, "resource_name")?,
        resource_attempt_id: optional_text(event, "resource_attempt_id")?,
        attempt_number: payload_attempt_number(&payload)?,
        operation_name: payload_text(&payload, "operation_name")?,
    })
}

pub(crate) fn context_variable(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    native_attr(py, NativeAttr::TerminalEventIndexContext)
}

/// The terminal event index installed for this invocation.
pub(crate) fn current_index(py: Python<'_>) -> PyResult<Option<PyRef<'_, TerminalEventIndex>>> {
    let installed: Bound<'_, PyAny> = context_variable(py)?.call_method0("get")?;
    if installed.is_none() {
        return Ok(None);
    }
    Ok(Some(installed.extract()?))
}

/// The installed terminal event index, as an owned reference.
pub(crate) fn current_index_object(py: Python<'_>) -> PyResult<Option<Py<TerminalEventIndex>>> {
    let installed: Bound<'_, PyAny> = context_variable(py)?.call_method0("get")?;
    if installed.is_none() {
        return Ok(None);
    }
    Ok(Some(installed.extract()?))
}

impl TerminalEventClaim {
    pub(crate) fn terminal_event<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        self.terminal.bind(py).clone()
    }

    pub(crate) fn sequence(&self) -> usize {
        self.event_sequence
    }
}
