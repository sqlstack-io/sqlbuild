//! `DiscoveredProjectInputs` with the loaded configuration, inside the project discovery lifecycle.

use crate::discovery::_helpers::project::configuration::configuration_inputs;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

/// `DiscoveredProjectInputs` with the loaded configuration, inside the project discovery lifecycle.
pub fn discover_project_configuration<'py>(
    py: Python<'py>,
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    configuration_inputs(py, project_dir, |_| Ok(()))
}
