//! Project graph.

use crate::planner_scope::_helpers::values;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_analysis::planner_scope::models::LoaderGraph;

/// Project graph.
#[inline]
pub fn project_graph(project: &Bound<'_, PyAny>) -> PyResult<LoaderGraph> {
    values::project_graph(project)
}
