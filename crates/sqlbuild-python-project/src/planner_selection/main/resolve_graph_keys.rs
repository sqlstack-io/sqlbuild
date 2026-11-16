//! Resolve graph keys for native planning without building a public frozenset.

use crate::planner_selection::_helpers::selectors;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::models::ProjectGraph;

/// Resolve graph keys for native planning without building a public frozenset.
#[inline]
pub fn resolve_graph_keys<'py>(
    graph: &ProjectGraph,
    (select, exclude): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Vec<ObjectKey>> {
    selectors::resolve_graph_keys(graph, (select, exclude))
}
