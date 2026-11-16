//! The Python node names `select` minus `exclude` resolve to; every node when unselected.

use crate::graph::_helpers::python_nodes::python_selection::planner_error;
use crate::graph::_helpers::python_nodes::python_selection::selection_graph;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyFrozenSet;
use sqlbuild_analysis::python_selection::main::resolve_python_node_selectors::resolve_python_node_selectors;
use sqlbuild_analysis::python_selection::models::PythonSelectionGraph;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;

/// The Python node names `select` minus `exclude` resolve to; every node when unselected.
pub fn python_node_names<'py>(
    graph: &Bound<'py, PyAny>,
    (select, exclude): (Vec<String>, Vec<String>),
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let py: Python<'py> = graph.py();
    let native: PythonSelectionGraph = selection_graph(graph)?;
    let names: Vec<String> =
        compiler_guard(|| Ok(resolve_python_node_selectors(&native, &select, &exclude)))?
            .map_err(|error| planner_error(py, error))?;
    PyFrozenSet::new(py, names)
}
