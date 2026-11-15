//! The cached value of `name` on `record`, if computed before.

use crate::records::_helpers::native_classes::fields;
use crate::records::models::NativeRecord;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The cached value of `name` on `record`, if computed before.
#[inline]
pub fn cached_value(record: &Bound<'_, NativeRecord>, name: &str) -> PyResult<Option<Py<PyAny>>> {
    fields::cached_value(record, name)
}
