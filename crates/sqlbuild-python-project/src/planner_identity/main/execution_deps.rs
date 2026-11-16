//! Execution dependencies kept native until a public graph or plan asks for its dict view.

use crate::planner_identity::_helpers::graph;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_analysis::assembly::project::types::ObjectKey;

/// Execution dependencies kept native until a public graph or plan asks for its dict view.
#[inline]
pub fn execution_deps(project: &Bound<'_, PyAny>) -> PyResult<Vec<(ObjectKey, Vec<ObjectKey>)>> {
    graph::execution_deps(project)
}
