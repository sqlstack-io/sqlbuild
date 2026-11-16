//! Register the python selection functions and classes.

use crate::graph::_helpers::python_nodes::python_selection::python_node_graph_indexes;
use crate::graph::_helpers::python_nodes::python_selection::resolve_python_node_selection;
use crate::graph::_helpers::python_nodes::python_selection::resolve_python_sql_run_selection;
use crate::graph::main::python_nodes::resolve_python_check_excludes::resolve_python_check_excludes;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the python selection functions and classes.
pub fn register_python_selection(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(resolve_python_node_selection, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_python_check_excludes, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_python_sql_run_selection, module)?)?;
    module.add_function(wrap_pyfunction!(python_node_graph_indexes, module)?)
}
