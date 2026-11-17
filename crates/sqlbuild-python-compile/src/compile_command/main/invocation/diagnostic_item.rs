//! One diagnostic as the report's JSON item.

use crate::compile_command::_helpers::invocation::report::diagnostic_json;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyDict;

/// One diagnostic as the report's JSON item.
pub fn diagnostic_item<'py>(diagnostic: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    diagnostic_json(diagnostic)
}
