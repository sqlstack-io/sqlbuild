//! In a delta compile that leaves errors to full compiles, stop once `failed` reports one.

use crate::compile_context::_helpers::delta;

/// In a delta compile that leaves errors to full compiles, stop once `failed` reports one.
#[inline]
pub fn stop_on_errors(failed: bool) -> pyo3::PyResult<()> {
    delta::stop_on_errors(failed)
}
