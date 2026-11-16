//! Whether an installed projector shows a visible operation with this name.

use crate::build_progress::_helpers::progress;
use crate::build_progress::models::NativeProgressProjector;
use pyo3::prelude::PyResult;

/// Whether an installed projector shows a visible operation with this name.
#[inline]
pub fn projector_operation_active(
    projector: &NativeProgressProjector,
    operation_name: &str,
) -> PyResult<bool> {
    progress::projector_operation_active(projector, operation_name)
}
