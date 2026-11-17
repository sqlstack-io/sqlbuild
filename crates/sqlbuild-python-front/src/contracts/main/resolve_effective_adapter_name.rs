//! Resolve the effective adapter name, allowing local override.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Resolve the effective adapter name, allowing local override.
#[pyfunction]
#[pyo3(signature = (*, project_config, local_config))]
pub fn resolve_effective_adapter_name<'py>(
    project_config: &Bound<'py, PyAny>,
    local_config: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let local: Bound<'py, PyAny> = native_field(local_config, "adapter")?;
    if local.is_none() {
        return native_field(project_config, "adapter");
    }
    Ok(local)
}
