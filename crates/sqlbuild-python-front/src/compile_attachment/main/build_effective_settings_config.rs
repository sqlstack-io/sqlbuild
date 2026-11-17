//! Build effective settings without compiling resources.

use crate::compile_attachment::main::build_effective_settings::build_effective_settings;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Build effective settings without compiling resources.
#[pyfunction]
#[pyo3(signature = (*, discovered_inputs))]
pub fn build_effective_settings_config<'py>(
    discovered_inputs: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    build_effective_settings(
        &native_field(discovered_inputs, "project_config")?,
        &native_field(discovered_inputs, "local_config")?,
    )
}
