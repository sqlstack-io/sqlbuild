//! The Python node names `sqb check` exclude tokens name, skipping tokens that match nothing.

use crate::graph::_helpers::python_nodes::python_selection::planner_error;
use crate::graph::_helpers::python_nodes::python_selection::selection_graph;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyFrozenSet;
use sqlbuild_analysis::python_selection::main::resolve_python_node_excludes::resolve_python_node_excludes;
use sqlbuild_analysis::python_selection::models::PythonSelectionGraph;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;

/// The Python node names `sqb check` exclude tokens name, skipping tokens that match nothing.
#[pyfunction]
#[pyo3(signature = (*, graph, exclude))]
pub fn resolve_python_check_excludes<'py>(
    graph: &Bound<'py, PyAny>,
    exclude: Vec<String>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let py: Python<'py> = graph.py();
    let native: PythonSelectionGraph = selection_graph(graph)?;
    let names: Vec<String> =
        compiler_guard(|| Ok(resolve_python_node_excludes(&native, &exclude)))?
            .map_err(|error| planner_error(py, error))?;
    PyFrozenSet::new(py, names)
}
