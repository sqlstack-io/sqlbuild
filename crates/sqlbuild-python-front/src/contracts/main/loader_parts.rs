//! Loader parts.

use crate::contracts::_helpers::spec_resolution;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Loader parts.
#[inline]
pub fn loader_parts<'py>(
    py: Python<'py>,
    destination: &str,
    (default_database, default_schema): (Option<Bound<'py, PyAny>>, Option<Bound<'py, PyAny>>),
) -> PyResult<Bound<'py, PyAny>> {
    spec_resolution::loader_parts(py, destination, (default_database, default_schema))
}
