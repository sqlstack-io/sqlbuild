//! The plan as one JSON document, `json.dumps(indent=2)`.

use crate::plan_output::_helpers::plan_json;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;

/// The plan as one JSON document, `json.dumps(indent=2)`.
#[inline]
pub fn format_plan_json(py: Python<'_>, options: Option<Bound<'_, PyDict>>) -> PyResult<String> {
    plan_json::format_plan_json(py, options)
}
