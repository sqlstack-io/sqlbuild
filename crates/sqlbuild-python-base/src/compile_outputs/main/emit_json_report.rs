//! The report as orjson writes it, or as `json.dumps(indent=2)` does when orjson rejects it.

use crate::compile_outputs::_helpers::json_report;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The report as orjson writes it, or as `json.dumps(indent=2)` does when orjson rejects it.
#[inline]
pub fn emit_json_report(report: &Bound<'_, PyAny>) -> PyResult<String> {
    json_report::emit_json_report(report)
}
