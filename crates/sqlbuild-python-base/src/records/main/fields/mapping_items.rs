//! `mapping.items()` for iteration: a dict yields its `(key, value)` pairs through the C API.

use crate::records::_helpers::native_classes::fields;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `mapping.items()` for iteration: a dict yields its `(key, value)` pairs through the C API.
#[inline]
pub fn mapping_items<'py>(mapping: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    fields::mapping_items(mapping)
}
