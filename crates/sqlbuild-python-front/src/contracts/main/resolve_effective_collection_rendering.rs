//! Resolve declaration rendering over project configuration.

use crate::contracts::_helpers::spec_resolution;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Resolve declaration rendering over project configuration.
#[inline]
pub fn resolve_effective_collection_rendering<'py>(
    project_config: &Bound<'py, PyAny>,
    declaration_override: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    spec_resolution::resolve_effective_collection_rendering(project_config, declaration_override)
}
