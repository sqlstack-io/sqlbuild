//! Cap policy, bounds and physical input evidence of future-cursor safety, or `None`.

use crate::plan_output::_helpers::safety;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Cap policy, bounds and physical input evidence of future-cursor safety, or `None`.
#[inline]
pub fn future_cursor_safety<'py>(evidence: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    safety::future_cursor_safety(evidence)
}
