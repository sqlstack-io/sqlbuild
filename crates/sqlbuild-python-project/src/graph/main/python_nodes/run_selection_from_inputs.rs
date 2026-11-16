//! `PythonSqlRunSelection` of run selectors over the project graph and discovered Python nodes.

use crate::graph::_helpers::python_nodes::python_node_graph;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `PythonSqlRunSelection` of run selectors over the project graph and discovered Python nodes.
#[inline]
pub fn run_selection_from_inputs<'py>(
    (select, exclude): (Vec<String>, Vec<String>),
    project_graph: &Bound<'py, PyAny>,
    discovered_inputs: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    python_node_graph::run_selection_from_inputs(
        (select, exclude),
        project_graph,
        discovered_inputs,
    )
}
