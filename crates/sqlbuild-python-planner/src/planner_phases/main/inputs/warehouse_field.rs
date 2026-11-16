//! One field of a planner warehouse state passed as a Python object.

use crate::planner_phases::_helpers::inputs::runtime;
use crate::planner_phases::models::PlannerWarehouseState;
use pyo3::prelude::{Bound, Py, PyAny, PyResult};

/// One field of a planner warehouse state passed as a Python object.
#[inline]
pub fn warehouse_field<'py>(
    warehouse: &Bound<'py, PyAny>,
    field: fn(&PlannerWarehouseState) -> &Py<PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    runtime::warehouse_field(warehouse, field)
}
