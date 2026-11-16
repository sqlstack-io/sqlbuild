//! Keys.

use crate::planner_scope::_helpers::values;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_analysis::assembly::project::types::ObjectKey;

/// Keys.
#[inline]
pub fn keys(values: &Bound<'_, PyAny>) -> PyResult<Vec<ObjectKey>> {
    values::keys(values)
}
