//! Register the native fingerprint hashing and storage functions.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_project::fingerprints::main::register_latest_rows::register_latest_rows;
use sqlbuild_python_project::fingerprints::main::register_query_hashes::register_query_hashes;
use sqlbuild_python_project::fingerprints::main::register_storage_io::register_storage_io;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_query_hashes(module)?;
    register_storage_io(module)?;
    register_latest_rows(module)
}
