//! Merge project settings with local developer overrides.

use crate::compile_attachment::_helpers::effective_config;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Merge project settings with local developer overrides.
#[inline]
pub fn build_effective_settings<'py>(
    project_config: &Bound<'py, PyAny>,
    local_config: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    effective_config::build_effective_settings(project_config, local_config)
}
