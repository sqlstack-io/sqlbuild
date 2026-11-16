use pyo3::prelude::{Bound, PyModule, PyResult};
use sqlbuild_python_project::references::main::register_relations::register_relations;
use sqlbuild_python_project::references::main::register_source_relations::register_source_relations;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_relations(module)?;
    register_source_relations(module)
}
