//! The registered adapter for `adapter_name`, resolved through the adapter registry.

use crate::command_runtime::_helpers::adapters;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The registered adapter for `adapter_name`, resolved through the adapter registry.
#[inline]
pub fn resolve_cli_adapter<'py>(
    adapter_name: &Bound<'py, PyAny>,
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    adapters::resolve_cli_adapter(adapter_name, project_dir)
}
