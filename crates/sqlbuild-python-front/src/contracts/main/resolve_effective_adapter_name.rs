//! Resolve the effective adapter name, allowing local override.

use crate::contracts::_helpers::spec_resolution;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Resolve the effective adapter name, allowing local override.
#[inline]
pub fn resolve_effective_adapter_name<'py>(
    project_config: &Bound<'py, PyAny>,
    local_config: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    spec_resolution::resolve_effective_adapter_name(project_config, local_config)
}
