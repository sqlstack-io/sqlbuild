//! The attached-audit gate edges and hook-read edges of one project, in Python's order.

use crate::references::_helpers::explicit_references;
use crate::references::models::GateRow;
use crate::references::models::HookRow;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The attached-audit gate edges and hook-read edges of one project, in Python's order.
#[inline]
pub fn native_execution_gate_edges(
    project: &Bound<'_, PyAny>,
) -> PyResult<(Vec<GateRow>, Vec<HookRow>)> {
    explicit_references::native_execution_gate_edges(project)
}
