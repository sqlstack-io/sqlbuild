//! A declared field of a native record read from its values; any other attribute, or object, by `getattr`.

use crate::records::_helpers::native_classes::fields;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// A declared field of a native record read from its values; any other attribute, or object, by `getattr`.
#[inline]
#[track_caller]
pub fn field<'py>(value: &Bound<'py, PyAny>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    fields::field(value, name)
}
