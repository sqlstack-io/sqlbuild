//! The native record class or string enum a pure-Rust crate names, failing as `getattr` would.

use crate::records::_helpers::native_classes::registry;
use crate::records::types::RecordId;
use pyo3::prelude::PyResult;

/// The native record class or string enum a pure-Rust crate names, failing as `getattr` would.
#[inline]
pub fn record_named(name: &str) -> PyResult<RecordId> {
    registry::record_named(name)
}
