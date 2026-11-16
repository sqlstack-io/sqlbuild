//! The latest fingerprint per node identity, read with the adapter-rendered latest-row SQL.

use crate::fingerprints::_helpers::storage_io;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyDict;

/// The latest fingerprint per node identity, read with the adapter-rendered latest-row SQL.
#[inline]
pub fn read_latest_fingerprints<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    storage_io::read_latest_fingerprints(request)
}
