//! Register `WarehouseAccessRefusedError` on the native module.

use crate::warehouse_access::_helpers::synced_copy;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register `WarehouseAccessRefusedError` on the native module.
#[inline]
pub fn register_synced_copy(module: &Bound<'_, PyModule>) -> PyResult<()> {
    synced_copy::register(module)
}
