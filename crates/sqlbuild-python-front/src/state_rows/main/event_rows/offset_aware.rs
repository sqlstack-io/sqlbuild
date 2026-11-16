//! Whether `value` has a `tzinfo` that reports an offset.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Whether `value` has a `tzinfo` that reports an offset.
pub fn offset_aware(value: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(!native_field(value, "tzinfo")?.is_none() && !value.call_method0("utcoffset")?.is_none())
}
