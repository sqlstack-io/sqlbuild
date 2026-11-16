//! Merge base project connection with the selected environment overrides.

use crate::compile_attachment::_helpers::effective_config::expanded_connection;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;

/// Merge base project connection with the selected environment overrides.
#[pyfunction]
#[pyo3(signature = (*, project_config, local_config, target_config, effective_vars))]
pub fn build_effective_connection<'py>(
    project_config: &Bound<'py, PyAny>,
    local_config: &Bound<'py, PyAny>,
    target_config: &Bound<'py, PyAny>,
    effective_vars: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    expanded_connection(
        (project_config, local_config),
        target_config,
        effective_vars,
        false,
    )
}
