//! Build intermediate source map.

use crate::planner_scope::_helpers::loaders::intermediate_map;
use crate::planner_scope::main::keys::keys;
use pyo3::prelude::{Bound, PyAny, PyResult, pyfunction};
use pyo3::types::PyDict;

#[pyfunction]
#[pyo3(signature = (*, project, selected_keys))]
pub fn build_intermediate_source_map<'py>(
    project: &Bound<'py, PyAny>,
    selected_keys: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    intermediate_map(project, &keys(selected_keys)?, false)
}
