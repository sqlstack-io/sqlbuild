use pyo3::prelude::{Bound, PyModule, PyResult};

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    sqlbuild_python_project::compiled_views::main::records::register_records::register_records(
        module,
    )
}
