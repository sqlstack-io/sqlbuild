//! Elapsed time between two Python datetimes.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};

/// Seconds from `from` to `to`, negative when `to` is earlier.
pub fn elapsed_seconds(from: &Bound<'_, PyAny>, to: &Bound<'_, PyAny>) -> PyResult<f64> {
    to.sub(from)?.call_method0("total_seconds")?.extract()
}
