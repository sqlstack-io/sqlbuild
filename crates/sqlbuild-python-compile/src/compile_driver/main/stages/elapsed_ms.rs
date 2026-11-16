//! Milliseconds since a `time.monotonic()` reading, truncated as `int(... * 1000)`.

use crate::compile_driver::main::stages::monotonic::monotonic;
use pyo3::prelude::{PyResult, Python};

/// Milliseconds since a `time.monotonic()` reading, truncated as `int(... * 1000)`.
pub fn elapsed_ms(py: Python<'_>, started: f64) -> PyResult<i64> {
    Ok(((monotonic(py)? - started) * 1000.0) as i64)
}
