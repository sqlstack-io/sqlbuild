//! A native string enum member from its value.

use crate::planner_phases::_helpers::objects;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_python_base::records::types::RecordId;

/// A native string enum member from its value.
#[inline]
pub fn native_member<'py>(
    py: Python<'py>,
    class: RecordId,
    value: &str,
) -> PyResult<Bound<'py, PyAny>> {
    objects::native_member(py, class, value)
}
