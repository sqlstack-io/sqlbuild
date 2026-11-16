//! `--select` minus `--exclude` on a native graph, adding the functions selected models need.

use crate::graph::models::NativeProjectGraph;
use crate::planner_selection::_helpers::selectors::resolved;
use pyo3::prelude::{Bound, PyAny, PyRef, PyResult};
use pyo3::types::PyFrozenSet;

/// `--select` minus `--exclude` on a native graph, adding the functions selected models need.
pub fn resolve_graph_selection<'py>(
    graph: PyRef<'py, NativeProjectGraph>,
    select: &Bound<'py, PyAny>,
    exclude: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    resolved(graph.graph(), (select, exclude))
}
