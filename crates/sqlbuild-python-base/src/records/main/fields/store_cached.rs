//! Keep `value` as the cached value of `name` on `record`.

use crate::records::_helpers::native_classes::fields;
use crate::records::models::NativeRecord;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Keep `value` as the cached value of `name` on `record`.
#[inline]
pub fn store_cached(
    record: &Bound<'_, NativeRecord>,
    name: &'static str,
    value: &Py<PyAny>,
) -> PyResult<()> {
    fields::store_cached(record, name, value)
}
