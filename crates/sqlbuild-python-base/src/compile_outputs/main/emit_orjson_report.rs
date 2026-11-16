//! The report as `orjson.dumps(report, option=OPT_INDENT_2)`, raising orjson's `TypeError`.

use crate::compile_outputs::_helpers::json_report;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The report as `orjson.dumps(report, option=OPT_INDENT_2)`, raising orjson's `TypeError`.
#[inline]
pub fn emit_orjson_report(report: &Bound<'_, PyAny>) -> PyResult<String> {
    json_report::emit_orjson_report(report)
}
