//! Register the cursor intrinsics functions and classes.

use crate::attachments::_helpers::cursor_intrinsics;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the cursor intrinsics functions and classes.
#[inline]
pub fn register_cursor_intrinsics(module: &Bound<'_, PyModule>) -> PyResult<()> {
    cursor_intrinsics::register(module)
}
