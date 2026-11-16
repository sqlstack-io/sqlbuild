//! `value.timestamp()` of a `datetime`.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};

/// `value.timestamp()` of a `datetime`.
pub fn timestamp(value: &Bound<'_, PyAny>) -> PyResult<f64> {
    value.call_method0("timestamp")?.extract()
}
