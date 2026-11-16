//! `--select` minus `--exclude` on a project graph, adding the functions selected models need.

use crate::graph::models::CompiledProjectGraph;
use crate::planner_selection::_helpers::selectors::resolved;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyFrozenSet;

/// `--select` minus `--exclude` on a project graph, adding the functions selected models need.
pub fn select_project_graph<'py>(
    graph: &Bound<'py, PyAny>,
    select: &Bound<'py, PyAny>,
    exclude: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let graph: &Bound<'py, CompiledProjectGraph> = graph.cast()?;
    resolved(
        graph.get().native(graph.py()).get().graph(),
        (select, exclude),
    )
}
