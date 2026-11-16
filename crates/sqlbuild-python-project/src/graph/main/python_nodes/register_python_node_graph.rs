//! Register the python node graph functions and classes.

use crate::graph::_helpers::python_nodes::python_node_graph;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the python node graph functions and classes.
#[inline]
pub fn register_python_node_graph(module: &Bound<'_, PyModule>) -> PyResult<()> {
    python_node_graph::register(module)
}
