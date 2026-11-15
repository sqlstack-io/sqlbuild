//! `mapping.values()` for iteration: a dict yields its values through the C API.

use crate::records::_helpers::native_classes::fields;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `mapping.values()` for iteration: a dict yields its values through the C API.
#[inline]
pub fn mapping_values<'py>(mapping: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    fields::mapping_values(mapping)
}
