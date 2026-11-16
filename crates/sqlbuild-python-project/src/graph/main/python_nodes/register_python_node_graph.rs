//! Register the python node graph functions and classes.

use crate::graph::_helpers::python_nodes::python_node_graph::build_python_node_graph;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the python node graph functions and classes.
pub fn register_python_node_graph(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_python_node_graph, module)?)?;
    Ok(())
}
