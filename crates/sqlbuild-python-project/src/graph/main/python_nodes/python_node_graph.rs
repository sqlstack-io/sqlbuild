//! The internal executable Python-node graph of discovered project inputs.

use crate::graph::_helpers::python_nodes::python_node_graph::graph_with_sets;
use crate::graph::_helpers::python_nodes::python_selection::planner_error;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_analysis::node_set_dependencies::models::ResolvedNode;
use sqlbuild_python_front::discovery::main::records::stored_node_sets::stored_node_sets;

/// The executable Python-node graph, each resolved `job(...)` and `tag(...)` an edge to its end nodes.
pub fn python_node_graph<'py>(
    discovered_inputs: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let sets: Vec<ResolvedNode> = match stored_node_sets(discovered_inputs)? {
        Some(Ok(resolved)) => resolved,
        Some(Err(error)) => return Err(planner_error(discovered_inputs.py(), error)),
        None => Vec::new(),
    };
    graph_with_sets(discovered_inputs, sets)
}
