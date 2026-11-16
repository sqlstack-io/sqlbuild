//! Write the planner's compiled output files under `target_dir`.

use crate::compile_command::_helpers::artifacts::target_writer::write_planned_target;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_python_base::boundary::models::KeywordArguments;

/// Write the planner's compiled output files under `target_dir`.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn write_compile_target<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "write_compile_target",
        options,
        (&["target_dir", "adapter", "plan_output"], &["manifest"]),
    )?;
    write_planned_target(
        &arguments.value("target_dir")?,
        &arguments.value("adapter")?,
        &arguments.value("plan_output")?,
        &arguments.value("manifest")?,
    )
}
