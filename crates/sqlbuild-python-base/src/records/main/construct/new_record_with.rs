//! A new `id` record built from `(field, value)` keyword pairs.

use crate::records::_helpers::native_classes::construct;
use crate::records::types::RecordId;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// A new `id` record built from `(field, value)` keyword pairs.
#[inline]
pub fn new_record_with<'py>(
    py: Python<'py>,
    id: RecordId,
    pairs: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    construct::new_record_with(py, id, pairs)
}
