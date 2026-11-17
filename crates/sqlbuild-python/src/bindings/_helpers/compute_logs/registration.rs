//! Register the native compute log storage.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_output::compute_logs::main::register_storage::register_storage;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_storage(module)
}
