//! Whether a cost-aware adapter reports a cost capability.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;

/// Whether `adapter` reports a cost capability; a failing report still counts as one.
pub fn reports_cost_capability(adapter: &Bound<'_, PyAny>) -> PyResult<bool> {
    let none: Bound<'_, PyAny> =
        host_entry(adapter.py(), ExecutorHost::CostCapability)?.getattr("NONE")?;
    Ok(match adapter.call_method0("cost_capability") {
        Ok(capability) => !capability.eq(none)?,
        Err(_) => true,
    })
}
