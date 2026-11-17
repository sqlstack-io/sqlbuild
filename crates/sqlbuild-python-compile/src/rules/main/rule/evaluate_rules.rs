//! Evaluate independent rule phases concurrently, then finalize their combined findings.

use crate::rules::_helpers::rule::rule_dispatch;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;

/// Evaluate independent rule phases concurrently, then finalize their combined findings.
#[inline]
pub fn evaluate_rules<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    rule_dispatch::evaluate_rules(py, options)
}
