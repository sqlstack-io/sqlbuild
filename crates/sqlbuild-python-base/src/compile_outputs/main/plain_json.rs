//! `json.dumps(value)`, raising what `json.dumps` raises.

use crate::compile_outputs::_helpers::json_report;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `json.dumps(value)`, raising what `json.dumps` raises.
#[inline]
pub fn plain_json(value: &Bound<'_, PyAny>) -> PyResult<String> {
    json_report::plain_json(value)
}
