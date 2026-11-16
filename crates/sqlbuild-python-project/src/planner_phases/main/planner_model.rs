//! A planner model dataclass.

use crate::planner_phases::_helpers::objects;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_python_base::records::types::RecordId;

/// A planner model dataclass.
#[inline]
pub fn planner_model<'py>(
    py: Python<'py>,
    name: RecordId,
    fields: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    objects::planner_model(py, name, fields)
}
