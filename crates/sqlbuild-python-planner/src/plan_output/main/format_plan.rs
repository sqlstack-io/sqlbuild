//! The text plan grouped by reason with inline detail, optionally closed by its completion line.

use crate::plan_output::_helpers::plan_text;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;

/// The text plan grouped by reason with inline detail, optionally closed by its completion line.
#[inline]
pub fn format_plan(py: Python<'_>, options: Option<Bound<'_, PyDict>>) -> PyResult<String> {
    plan_text::format_plan(py, options)
}
