//! One model's change and backfill.

use crate::planner_changes::_helpers::detect;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;

/// One model's change and backfill.
#[inline]
pub fn detect_model_changes<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    detect::detect_model_changes(py, options)
}
