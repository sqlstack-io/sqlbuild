//! Serialize SQL and Python hook entries; other values serialize to no hooks.

use crate::discovery::_helpers::headers::hook_entries;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;
use pyo3::types::PyList;

/// Serialize SQL and Python hook entries; other values serialize to no hooks.
#[inline]
pub fn hook_entries<'py>(
    value: &Bound<'py, PyAny>,
    sql_fields: &[String],
    python_hook_fields: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyList>> {
    hook_entries::hook_entries(value, sql_fields, python_hook_fields)
}
