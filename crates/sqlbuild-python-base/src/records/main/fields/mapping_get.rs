//! `mapping.get(key)`: a dict is read through the C API.

use crate::records::_helpers::native_classes::fields;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `mapping.get(key)`: a dict is read through the C API.
#[inline]
pub fn mapping_get<'py, K: pyo3::IntoPyObject<'py>>(
    mapping: &Bound<'py, PyAny>,
    key: K,
) -> PyResult<Bound<'py, PyAny>> {
    fields::mapping_get(mapping, key)
}
