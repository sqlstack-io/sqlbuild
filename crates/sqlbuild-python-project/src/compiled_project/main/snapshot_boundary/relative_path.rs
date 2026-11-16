//! Relative path.

use crate::compiled_project::_helpers::snapshot_boundary;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Relative path.
#[inline]
pub fn relative_path(value: &Bound<'_, PyAny>) -> PyResult<String> {
    snapshot_boundary::relative_path(value)
}
