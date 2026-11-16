//! Whether the adapter can switch warehouses per session, as the adapter host answers.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_host::entries::models::AdapterHost;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;

/// Whether the adapter can switch warehouses per session, as the adapter host answers.
pub fn session_warehouse_support(
    py: Python<'_>,
    adapter_name: &Bound<'_, PyAny>,
    project_dir: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    call_with(
        &host_entry(py, AdapterHost::AdapterSupportsSessionWarehouse)?,
        &[
            ("adapter_name", adapter_name.clone()),
            ("project_dir", project_dir.clone()),
        ],
    )?
    .is_truthy()
}
