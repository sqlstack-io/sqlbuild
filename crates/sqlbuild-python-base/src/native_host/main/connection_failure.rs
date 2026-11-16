//! The coded error to raise for a failed `adapter.connect`, or the failure itself when coded.

use crate::native_host::_helpers::connections;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyErr;

/// The coded error to raise for a failed `adapter.connect`, or the failure itself when coded.
#[inline]
pub fn connection_failure(adapter: &Bound<'_, PyAny>, error: &PyErr) -> PyErr {
    connections::connection_failure(adapter, error)
}
