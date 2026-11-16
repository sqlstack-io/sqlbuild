//! The validated configuration without a discovery lifecycle, which reading two files never needs.

use crate::discovery::_helpers::project::configuration;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The validated configuration without a discovery lifecycle, which reading two files never needs.
#[inline]
pub fn discover_connection_configuration<'py>(
    py: Python<'py>,
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    configuration::discover_connection_configuration(py, project_dir)
}
