//! Build effective settings without compiling resources.

use crate::compile_attachment::_helpers::effective_config;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Build effective settings without compiling resources.
#[inline]
pub fn build_effective_settings_config<'py>(
    discovered_inputs: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    effective_config::build_effective_settings_config(discovered_inputs)
}
