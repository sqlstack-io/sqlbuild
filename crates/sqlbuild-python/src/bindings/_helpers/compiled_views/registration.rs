use pyo3::prelude::{Bound, PyModule, PyResult};
use sqlbuild_python_project::compiled_views::main::project::register_records::register_records;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_records(module)
}
