//! Build effective target config without compiling resources.

use crate::compile_attachment::main::target_selection::target_selection;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;

/// Build effective target config without compiling resources.
#[pyfunction]
#[pyo3(signature = (*, discovered_inputs, selected_target = None))]
pub fn build_effective_target_config<'py>(
    discovered_inputs: &Bound<'py, PyAny>,
    selected_target: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    Ok(target_selection(discovered_inputs, selected_target)?.1)
}
