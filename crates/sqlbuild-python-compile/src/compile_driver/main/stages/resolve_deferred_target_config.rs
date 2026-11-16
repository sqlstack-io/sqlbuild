//! Validate and resolve the deferred target config.

use crate::compile_driver::_helpers::stages::deferral::deferred_target_config;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_python_base::boundary::models::KeywordArguments;

/// Validate and resolve the deferred target config.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn resolve_deferred_target_config<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "resolve_deferred_target_config",
        options,
        (
            &["discovered_inputs", "defer_to", "current_target_name"],
            &[],
        ),
    )?;
    deferred_target_config(
        &arguments.value("discovered_inputs")?,
        &arguments.value("defer_to")?,
        &arguments.value("current_target_name")?,
    )
}
