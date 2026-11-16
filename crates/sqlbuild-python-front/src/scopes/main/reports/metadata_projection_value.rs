//! `plain_value` of `scope_metadata_projection(index=index)`, without a Python round trip.

use crate::scopes::_helpers::reports::scope_reports;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `plain_value` of `scope_metadata_projection(index=index)`, without a Python round trip.
#[inline]
pub fn metadata_projection_value(index: &Bound<'_, PyAny>) -> PyResult<serde_json::Value> {
    scope_reports::metadata_projection_value(index)
}
