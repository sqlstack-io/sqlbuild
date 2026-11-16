//! A member of a natively declared planner enumeration, from its value.

use crate::planner_changes::_helpers::results;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_python_base::records::types::RecordId;

/// A member of a natively declared planner enumeration, from its value.
#[inline]
pub fn native_enum<'py>(
    py: Python<'py>,
    name: RecordId,
    value: &str,
) -> PyResult<Bound<'py, PyAny>> {
    results::native_enum(py, name, value)
}
