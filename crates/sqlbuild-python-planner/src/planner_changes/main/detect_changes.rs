//! Selected model and function changes against expected version hashes and metadata JSON.

use crate::planner_changes::_helpers::detect;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Selected model and function changes against expected version hashes and metadata JSON.
#[inline]
pub fn detect_changes<'py>(
    (project, scope, snapshot): (&Bound<'py, PyAny>, &Bound<'py, PyAny>, &Bound<'py, PyAny>),
    full_refresh: bool,
    expected: (Bound<'py, PyAny>, Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    detect::detect_changes((project, scope, snapshot), full_refresh, expected)
}
