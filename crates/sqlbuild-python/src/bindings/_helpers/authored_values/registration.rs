use pyo3::prelude::{Bound, PyModule, PyResult};

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    sqlbuild_python_front::authored_values::main::register_fields::register_fields(module)?;
    sqlbuild_python_front::authored_values::main::register_project_vars::register_project_vars(
        module,
    )?;
    sqlbuild_python_front::authored_values::main::register_python_nodes::register_python_nodes(
        module,
    )
}
