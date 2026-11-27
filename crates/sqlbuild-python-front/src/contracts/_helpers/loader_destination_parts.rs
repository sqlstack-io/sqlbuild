//! Parse a one-, two-, or three-part loader destination.

use crate::contracts::main::loader_parts::loader_parts;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;

/// Parse a one-, two-, or three-part loader destination.
#[pyfunction]
#[pyo3(signature = (*, destination, default_database, default_schema))]
pub(crate) fn loader_destination_parts<'py>(
    py: Python<'py>,
    destination: &str,
    default_database: Option<Bound<'py, PyAny>>,
    default_schema: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    loader_parts(py, destination, (default_database, default_schema))
}
