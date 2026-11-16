//! Merge base project connection with the selected environment overrides.

use crate::compile_attachment::_helpers::effective_config;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Merge base project connection with the selected environment overrides.
#[inline]
pub fn build_effective_connection<'py>(
    project_config: &Bound<'py, PyAny>,
    local_config: &Bound<'py, PyAny>,
    target_config: &Bound<'py, PyAny>,
    effective_vars: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    effective_config::build_effective_connection(
        project_config,
        local_config,
        target_config,
        effective_vars,
    )
}
