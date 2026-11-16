//! `--select` minus `--exclude` on a native graph, adding the functions selected models need.

use crate::graph::models::NativeProjectGraph;
use crate::planner_selection::_helpers::selectors;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyRef;
use pyo3::prelude::PyResult;
use pyo3::types::PyFrozenSet;

/// `--select` minus `--exclude` on a native graph, adding the functions selected models need.
#[inline]
pub fn resolve_graph_selection<'py>(
    graph: PyRef<'py, NativeProjectGraph>,
    select: &Bound<'py, PyAny>,
    exclude: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    selectors::resolve_graph_selection(graph, select, exclude)
}
