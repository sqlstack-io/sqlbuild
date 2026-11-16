//! Whether `error` is a plan preview reaching past the synced copy for the warehouse.

use crate::warehouse_access::_helpers::synced_copy;
use pyo3::PyErr;
use pyo3::prelude::Python;

/// Whether `error` is a plan preview reaching past the synced copy for the warehouse.
#[inline]
pub fn is_warehouse_access_refused(py: Python<'_>, error: &PyErr) -> bool {
    synced_copy::is_warehouse_access_refused(py, error)
}
