//! The validated configuration without a discovery lifecycle, which reading two files never needs.

use crate::discovery::_helpers::project::configuration::loaded_configuration;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

/// The validated configuration without a discovery lifecycle, which reading two files never needs.
pub fn discover_connection_configuration<'py>(
    py: Python<'py>,
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    loaded_configuration(py, project_dir, true)
}
