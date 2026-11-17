//! Build effective target config without compiling resources.

use crate::compile_attachment::_helpers::effective_config;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Build effective target config without compiling resources.
#[inline]
pub fn build_effective_target_config<'py>(
    discovered_inputs: &Bound<'py, PyAny>,
    selected_target: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    effective_config::build_effective_target_config(discovered_inputs, selected_target)
}
