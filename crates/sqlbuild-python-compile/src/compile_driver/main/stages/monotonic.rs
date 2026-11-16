//! A `time.monotonic()` reading.

use pyo3::prelude::{PyAnyMethods, PyResult, Python};
use sqlbuild_python_base::boundary::main::imported::imported;

/// A `time.monotonic()` reading.
pub fn monotonic(py: Python<'_>) -> PyResult<f64> {
    imported(py, "time", "monotonic")?.call0()?.extract()
}
