//! Fail module init when a `NativeAttr` names an attribute the finished module lacks.

use crate::records::_helpers::native_classes::registry;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Fail module init when a `NativeAttr` names an attribute the finished module lacks.
#[inline]
pub fn verify_attributes(module: &Bound<'_, PyModule>) -> PyResult<()> {
    registry::verify_attributes(module)
}
