//! JSON-backed configuration keeps integer widths and recursively constructs native Python values.

use crate::boundary::_helpers::config_values;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// JSON-backed configuration keeps integer widths and recursively constructs native Python values.
#[inline]
pub fn json_value_to_python<'py>(
    py: Python<'py>,
    value: &serde_json::Value,
) -> PyResult<Bound<'py, PyAny>> {
    config_values::json_value_to_python(py, value)
}
