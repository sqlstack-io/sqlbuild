//! `json.dumps` writes a float key as `float.__repr__`, or as `NaN` / `Infinity`.

use crate::compile_outputs::_helpers::json_report;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `json.dumps` writes a float key as `float.__repr__`, or as `NaN` / `Infinity`.
#[inline]
pub fn float_key(key: &Bound<'_, PyAny>) -> PyResult<String> {
    json_report::float_key(key)
}
