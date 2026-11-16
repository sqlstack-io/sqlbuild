//! Resolve the effective target name.

use crate::contracts::_helpers::spec_resolution;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyString;

/// Resolve the effective target name.
#[inline]
pub fn resolve_target_name<'py>(
    py: Python<'py>,
    project_config: &Bound<'py, PyAny>,
    local_config: &Bound<'py, PyAny>,
    selected_target: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyString>>> {
    spec_resolution::resolve_target_name(py, project_config, local_config, selected_target)
}
