//! Time since a Python datetime, by the UTC clock now.

use crate::cursor::constants::DATETIME_MODULE;
use crate::native_host::main::elapsed_seconds::elapsed_seconds;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_python_base::boundary::main::imported::imported;

/// Seconds since `value`, by the UTC clock now.
pub fn seconds_since(value: &Bound<'_, PyAny>) -> PyResult<f64> {
    let py: Python<'_> = value.py();
    let now: Bound<'_, PyAny> = imported(py, DATETIME_MODULE, "datetime")?
        .call_method1("now", (imported(py, DATETIME_MODULE, "UTC")?,))?;
    elapsed_seconds(value, &now)
}
