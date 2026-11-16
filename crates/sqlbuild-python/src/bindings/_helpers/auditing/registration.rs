use pyo3::prelude::{Bound, PyModule, PyResult};
use sqlbuild_python_compile::auditing::main::register_identity::register_identity;
use sqlbuild_python_compile::auditing::main::register_measurement::register_measurement;
use sqlbuild_python_compile::auditing::main::register_schema_audits::register_schema_audits;
use sqlbuild_python_project::auditing::main::register_render::register_render;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_identity(module)?;
    register_measurement(module)?;
    register_render(module)?;
    register_schema_audits(module)
}
