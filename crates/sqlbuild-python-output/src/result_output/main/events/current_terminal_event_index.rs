//! Return the terminal event index installed for this invocation.

use crate::result_output::_helpers::events::terminal_index::context_variable;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;

/// Return the terminal event index installed for this invocation.
#[pyfunction]
pub fn current_terminal_event_index(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    context_variable(py)?.call_method0("get")
}
