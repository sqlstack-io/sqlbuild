//! The type object of the native pyclass `T`, as a getter table entry names it.

use crate::records::_helpers::native_classes::getters;
use pyo3::PyTypeInfo;
use pyo3::prelude::Bound;
use pyo3::prelude::Python;
use pyo3::types::PyType;

/// The type object of the native pyclass `T`, as a getter table entry names it.
#[inline]
pub fn class_of<'py, T: PyTypeInfo>(py: Python<'py>) -> Bound<'py, PyType> {
    getters::class_of::<T>(py)
}
