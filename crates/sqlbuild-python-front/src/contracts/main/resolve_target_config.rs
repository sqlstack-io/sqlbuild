//! Merge project target config with local developer overrides.

use crate::contracts::_helpers::spec_resolution;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Merge project target config with local developer overrides.
#[inline]
pub fn resolve_target_config<'py>(
    py: Python<'py>,
    project_config: &Bound<'py, PyAny>,
    local_config: &Bound<'py, PyAny>,
    target_name: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    spec_resolution::resolve_target_config(py, project_config, local_config, target_name)
}
