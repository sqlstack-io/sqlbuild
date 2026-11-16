//! Maximum allowed start.

use crate::planner_resolution::_helpers::safety::maximum_safety;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;

/// Maximum allowed start.
#[inline]
pub fn maximum_allowed_start<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<String> {
    maximum_safety::maximum_allowed_start(py, kwargs)
}
