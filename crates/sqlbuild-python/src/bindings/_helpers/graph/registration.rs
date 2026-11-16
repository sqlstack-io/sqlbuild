use pyo3::prelude::{Bound, PyModule, PyResult};
use sqlbuild_python_project::graph::main::nodes::register_algorithms::register_algorithms;
use sqlbuild_python_project::graph::main::nodes::register_execution_gates::register_execution_gates;
use sqlbuild_python_project::graph::main::project::register_project_graph::register_project_graph;
use sqlbuild_python_project::graph::main::project::register_views::register_views;
use sqlbuild_python_project::graph::main::python_nodes::register_python_lifecycle::register_python_lifecycle;
use sqlbuild_python_project::graph::main::python_nodes::register_python_node_graph::register_python_node_graph;
use sqlbuild_python_project::graph::main::python_nodes::register_python_selection::register_python_selection;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_project_graph(module)?;
    register_algorithms(module)?;
    register_execution_gates(module)?;
    register_python_selection(module)?;
    register_python_lifecycle(module)?;
    register_python_node_graph(module)?;
    register_views(module)
}
