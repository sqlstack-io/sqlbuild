//! `plain_value` of `scope_metadata_projection(index=index)`, without a Python round trip.

use crate::scopes::_helpers::reports::scope_report_objects::json_to_plain;
use crate::scopes::_helpers::reports::scope_reports::projection_json;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// `plain_value` of `scope_metadata_projection(index=index)`, without a Python round trip.
pub fn metadata_projection_value(index: &Bound<'_, PyAny>) -> PyResult<serde_json::Value> {
    json_to_plain(projection_json(index)?)
}
