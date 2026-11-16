use pyo3::prelude::{Bound, PyModule, PyResult};

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    sqlbuild_python_project::references::main::register_relations::register_relations(module)?;
    sqlbuild_python_project::references::main::register_source_relations::register_source_relations(
        module,
    )
}
