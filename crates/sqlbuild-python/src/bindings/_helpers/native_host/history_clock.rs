//! Elapsed time between query-history datetimes and an adapter's cost capability, read across the Python boundary.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::boundary::python_calls::imported;
use crate::bindings::_helpers::native_host::host;

const DATETIME_MODULE: &str = "datetime";

/// Seconds from `from` to `to`, negative when `to` is earlier.
pub(crate) fn elapsed_seconds(from: &Bound<'_, PyAny>, to: &Bound<'_, PyAny>) -> PyResult<f64> {
    to.sub(from)?.call_method0("total_seconds")?.extract()
}

/// Seconds since `value`, by the UTC clock now.
pub(crate) fn seconds_since(value: &Bound<'_, PyAny>) -> PyResult<f64> {
    let py: Python<'_> = value.py();
    let now: Bound<'_, PyAny> = imported(py, DATETIME_MODULE, "datetime")?
        .call_method1("now", (imported(py, DATETIME_MODULE, "UTC")?,))?;
    elapsed_seconds(value, &now)
}

/// Whether a cost-aware adapter reports a cost capability; a failing report still counts as one.
pub(crate) fn reports_cost_capability(adapter: &Bound<'_, PyAny>) -> PyResult<bool> {
    let none: Bound<'_, PyAny> =
        host::entry(adapter.py(), ExecutorHost::CostCapability)?.getattr("NONE")?;
    Ok(match adapter.call_method0("cost_capability") {
        Ok(capability) => !capability.eq(none)?,
        Err(_) => true,
    })
}
