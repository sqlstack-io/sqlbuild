//! Resolve declaration rendering over project configuration.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Resolve declaration rendering over project configuration.
#[pyfunction]
#[pyo3(signature = (*, project_config, declaration_override))]
pub fn resolve_effective_collection_rendering<'py>(
    project_config: &Bound<'py, PyAny>,
    declaration_override: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    if !declaration_override.is_none() {
        return Ok(declaration_override.clone());
    }
    native_field(project_config, "constants")?.getattr("collection_rendering")
}
