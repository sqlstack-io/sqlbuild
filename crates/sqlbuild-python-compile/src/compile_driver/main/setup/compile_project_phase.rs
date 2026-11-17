//! Compile a project without opening a warehouse planning connection.

use crate::compile_driver::main::setup::compiled_phase::compiled_phase;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_python_base::boundary::models::KeywordArguments;

/// Compile a project without opening a warehouse planning connection.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn compile_project_phase<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "compile_project_phase",
        options,
        (
            &["discovered_inputs", "adapter"],
            &["options", "on_progress"],
        ),
    )?;
    compiled_phase(
        (
            &arguments.value("discovered_inputs")?,
            &arguments.value("adapter")?,
        ),
        &arguments.value("options")?,
        &arguments.value("on_progress")?,
        None,
    )
}
