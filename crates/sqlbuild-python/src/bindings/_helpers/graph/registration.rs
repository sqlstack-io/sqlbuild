use pyo3::prelude::{Bound, PyModule, PyResult};

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    sqlbuild_python_project::graph::main::project_graph::register_project_graph::register_project_graph(module)?;
    sqlbuild_python_project::graph::main::algorithms::register_algorithms::register_algorithms(
        module,
    )?;
    sqlbuild_python_project::graph::main::execution_gates::register_execution_gates::register_execution_gates(module)?;
    sqlbuild_python_project::graph::main::python_selection::register_python_selection::register_python_selection(module)?;
    sqlbuild_python_project::graph::main::python_lifecycle::register_python_lifecycle::register_python_lifecycle(module)?;
    sqlbuild_python_project::graph::main::python_node_graph::register_python_node_graph::register_python_node_graph(module)?;
    sqlbuild_python_project::graph::main::views::register_views::register_views(module)
}
