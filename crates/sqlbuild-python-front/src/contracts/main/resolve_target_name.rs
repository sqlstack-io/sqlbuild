//! Resolve the effective target name.

use crate::contracts::_helpers::effective_config::effective_config_target_name;
use crate::contracts::_helpers::spec_resolution::spec_error;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyString;

/// Resolve the effective target name.
#[pyfunction]
#[pyo3(signature = (*, project_config, local_config, selected_target))]
pub fn resolve_target_name<'py>(
    py: Python<'py>,
    project_config: &Bound<'py, PyAny>,
    local_config: &Bound<'py, PyAny>,
    selected_target: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyString>>> {
    let (name, error) =
        effective_config_target_name(py, project_config, local_config, selected_target)?;
    match error {
        Some(message) => Err(spec_error(py, message)?),
        None => Ok(name),
    }
}
