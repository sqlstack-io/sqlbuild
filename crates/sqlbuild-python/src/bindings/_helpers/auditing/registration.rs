use pyo3::prelude::{Bound, PyModule, PyResult};
use sqlbuild_python_project::auditing::main::register_render::register_render;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    crate::bindings::_helpers::auditing::identity::register(module)?;
    crate::bindings::_helpers::auditing::measurement::register(module)?;
    register_render(module)?;
    crate::bindings::_helpers::auditing::schema_audits::register(module)
}
