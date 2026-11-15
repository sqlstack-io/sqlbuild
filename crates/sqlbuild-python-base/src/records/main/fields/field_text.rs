//! The text of a record's string field `name`.

use crate::records::_helpers::native_classes::fields;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The text of a record's string field `name`.
#[inline]
pub fn field_text(value: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    fields::field_text(value, name)
}
