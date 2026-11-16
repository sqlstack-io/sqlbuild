//! A JSON value as Python builds it; arrays become tuples or lists.

use crate::scopes::_helpers::reports::scope_report_objects;
use crate::scopes::_helpers::reports::scope_report_objects::PyObject;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_core::json::models::JsonValue;

/// A JSON value as Python builds it; arrays become tuples or lists.
#[inline]
pub fn json_to_python(py: Python<'_>, value: JsonValue, tuples: bool) -> PyResult<PyObject> {
    scope_report_objects::json_to_python(py, value, tuples)
}
