//! Run the selected built-in, SQL and custom rules; custom rules run on their worker threads.

use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;

/// Run the selected built-in, SQL and custom rules; custom rules run on their worker threads.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn run_rules<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    crate::rules::_helpers::rule::rule_dispatch::evaluate_rules(py, options)
}
