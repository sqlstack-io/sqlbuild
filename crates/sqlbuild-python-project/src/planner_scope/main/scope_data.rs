//! Read native ownership directly, or import a caller-authored public scope at the API edge.

use crate::planner_scope::_helpers::retained;
use crate::planner_scope::models::ScopeData;
use pyo3::prelude::{Bound, PyAny, PyResult};
use std::sync::Arc;

/// Read native ownership directly, or import a caller-authored public scope at the API edge.
#[inline]
pub fn scope_data(value: &Bound<'_, PyAny>) -> PyResult<Arc<ScopeData>> {
    retained::scope_data(value)
}
