//! `--select` minus `--exclude` on a project graph, adding the functions selected models need.

use crate::planner_selection::_helpers::selectors;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyFrozenSet;

/// `--select` minus `--exclude` on a project graph, adding the functions selected models need.
#[inline]
pub fn select_project_graph<'py>(
    graph: &Bound<'py, PyAny>,
    select: &Bound<'py, PyAny>,
    exclude: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    selectors::select_project_graph(graph, select, exclude)
}
