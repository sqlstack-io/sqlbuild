//! Select source entries from native reachability; the resulting entries cross the executor edge.

use crate::planner_scope::_helpers::sources;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;
use sqlbuild_analysis::assembly::project::types::ObjectKey;

/// Select source entries from native reachability; the resulting entries cross the executor edge.
#[inline]
pub fn source_load_map<'py>(
    project: &Bound<'py, PyAny>,
    selected: &[ObjectKey],
    upstream_deps: Option<&[(ObjectKey, Vec<ObjectKey>)]>,
) -> PyResult<Bound<'py, PyDict>> {
    sources::source_load_map(project, selected, upstream_deps)
}
