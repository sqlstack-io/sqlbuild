//! Whether the adapter can switch warehouses per session, as the adapter host answers.

use crate::compile_driver::_helpers::config_validation;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Whether the adapter can switch warehouses per session, as the adapter host answers.
#[inline]
pub fn session_warehouse_support(
    py: Python<'_>,
    adapter_name: &Bound<'_, PyAny>,
    project_dir: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    config_validation::session_warehouse_support(py, adapter_name, project_dir)
}
