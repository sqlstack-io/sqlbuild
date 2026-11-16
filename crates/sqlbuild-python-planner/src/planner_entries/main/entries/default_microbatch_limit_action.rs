//! The microbatch limit action used when a project configures none.

use crate::planner_entries::_helpers::entries::contexts;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The microbatch limit action used when a project configures none.
#[inline]
pub fn default_microbatch_limit_action(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    contexts::default_microbatch_limit_action(py)
}
