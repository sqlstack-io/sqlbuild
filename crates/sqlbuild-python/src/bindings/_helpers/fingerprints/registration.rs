//! Register the native fingerprint hashing and storage functions.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::fingerprints::latest_rows;
use crate::bindings::_helpers::fingerprints::query_hashes;
use crate::bindings::_helpers::fingerprints::storage_io;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    query_hashes::register(module)?;
    storage_io::register(module)?;
    latest_rows::register(module)
}
