//! One field of a frozen native planner class passed as a Python object.

use crate::planner_phases::_helpers::inputs::contexts;
use pyo3::PyClass;
use pyo3::prelude::{Bound, Py, PyAny, PyResult};
use pyo3::pyclass::boolean_struct::True;

/// One field of a frozen native planner class passed as a Python object.
#[inline]
pub fn class_field<'py, T>(
    value: &Bound<'py, PyAny>,
    field: fn(&T) -> &Py<PyAny>,
) -> PyResult<Bound<'py, PyAny>>
where
    T: PyClass<Frozen = True> + Sync,
{
    contexts::class_field(value, field)
}
