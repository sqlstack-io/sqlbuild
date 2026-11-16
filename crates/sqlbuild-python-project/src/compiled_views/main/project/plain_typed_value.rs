//! A typed SQL value as the plain value native consumers read.

use crate::compiled_views::_helpers::project::records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// A typed SQL value as the plain value native consumers read.
#[inline]
pub fn plain_typed_value<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    records::plain_typed_value(value)
}
