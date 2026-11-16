//! Build upstream intermediate source map.

use crate::planner_scope::_helpers::loaders::intermediate_map;
use crate::planner_scope::main::keys::keys;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyDict;

/// Build upstream intermediate source map.
pub fn build_upstream_intermediate_source_map<'py>(
    project: &Bound<'py, PyAny>,
    selected_keys: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    intermediate_map(project, &keys(selected_keys)?, true)
}
