//! The internal executable Python-node graph of discovered project inputs.

use crate::graph::_helpers::python_nodes::python_node_graph;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The internal executable Python-node graph of discovered project inputs.
#[inline]
pub fn python_node_graph<'py>(
    discovered_inputs: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    python_node_graph::python_node_graph(discovered_inputs)
}
