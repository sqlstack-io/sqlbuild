//! `build_effective_vars` for a discovered project and target, without CLI vars.

use crate::compile_attachment::_helpers::effective_config;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `build_effective_vars` for a discovered project and target, without CLI vars.
#[inline]
pub fn project_vars<'py>(
    discovered: &Bound<'py, PyAny>,
    target_config: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    effective_config::project_vars(discovered, target_config)
}
