//! Edges.

use crate::planner_scope::_helpers::values;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_analysis::assembly::project::types::ObjectKey;

/// Edges.
#[inline]
pub fn edges(values: &Bound<'_, PyAny>) -> PyResult<Vec<(ObjectKey, Vec<ObjectKey>)>> {
    values::edges(values)
}
