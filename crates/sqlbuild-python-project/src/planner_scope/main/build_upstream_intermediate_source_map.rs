//! Build upstream intermediate source map.

use crate::planner_scope::_helpers::loaders;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// Build upstream intermediate source map.
#[inline]
pub fn build_upstream_intermediate_source_map<'py>(
    project: &Bound<'py, PyAny>,
    selected_keys: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    loaders::build_upstream_intermediate_source_map(project, selected_keys)
}
