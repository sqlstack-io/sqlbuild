//! `getattr(value, name, None)` of a native record, `None` when `value` is not one.

use crate::records::_helpers::native_classes::fields;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `getattr(value, name, None)` of a native record, `None` when `value` is not one.
#[inline]
pub fn record_attribute_opt<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Option<Bound<'py, PyAny>>>> {
    fields::record_attribute_opt(value, name)
}
