//! The compiled object key of the model `name`.

use crate::planner_phases::_helpers::objects;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The compiled object key of the model `name`.
#[inline]
pub fn model_key<'py>(py: Python<'py>, name: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    objects::model_key(py, name)
}
