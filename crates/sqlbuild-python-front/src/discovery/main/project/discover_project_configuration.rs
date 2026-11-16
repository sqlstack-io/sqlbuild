//! `DiscoveredProjectInputs` with the loaded configuration, inside the project discovery lifecycle.

use crate::discovery::_helpers::project::configuration;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// `DiscoveredProjectInputs` with the loaded configuration, inside the project discovery lifecycle.
#[inline]
pub fn discover_project_configuration<'py>(
    py: Python<'py>,
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    configuration::discover_project_configuration(py, project_dir)
}
