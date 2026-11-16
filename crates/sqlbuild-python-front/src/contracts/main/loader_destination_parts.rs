//! Parse a one-, two-, or three-part loader destination.

use crate::contracts::_helpers::spec_resolution;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Parse a one-, two-, or three-part loader destination.
#[inline]
pub fn loader_destination_parts<'py>(
    py: Python<'py>,
    destination: &str,
    default_database: Option<Bound<'py, PyAny>>,
    default_schema: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    spec_resolution::loader_destination_parts(py, destination, default_database, default_schema)
}
