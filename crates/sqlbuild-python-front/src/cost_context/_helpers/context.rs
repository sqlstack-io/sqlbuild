//! The `ContextVar` that holds the current cost resource context, and the scopes built over it.

use crate::cost_context::main::process_object::process_object;
use crate::cost_context::models::CostScope;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::PyDict;

use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const CONTEXT_VARIABLE_NAME: &str = "sqlbuild_cost_resource_context";
pub(crate) const DEFAULT_PHASE: &str = "execute";
pub(crate) const DEFAULT_ATTEMPT: i64 = 1;

static CURRENT: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

/// The `ContextVar` holding the current cost resource context.
pub(crate) fn current_variable(py: Python<'_>) -> PyResult<&Bound<'_, PyAny>> {
    process_object(&CURRENT, py, || {
        let options: Bound<'_, PyDict> = PyDict::new(py);
        options.set_item("default", py.None())?;
        imported(py, "contextvars", "ContextVar")?.call((CONTEXT_VARIABLE_NAME,), Some(&options))
    })
}

pub(crate) fn or_current<'py>(
    value: Bound<'py, PyAny>,
    current: &Bound<'py, PyAny>,
    field: &str,
) -> PyResult<Bound<'py, PyAny>> {
    if !value.is_none() || current.is_none() {
        return Ok(value);
    }
    native_field(current, field)
}

pub(crate) fn scope_of(context: Bound<'_, PyAny>) -> CostScope {
    CostScope {
        context: Some(context.unbind()),
        token: None,
    }
}

/// The active `CostResourceContext`, or `None`.
pub(crate) fn current_context(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    current_variable(py)?.call_method0("get")
}
