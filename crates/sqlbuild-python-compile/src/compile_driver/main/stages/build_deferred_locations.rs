//! Build physical locations for all models and seeds under a deferred target.

use crate::compile_driver::_helpers::stages::deferral::DeferredNaming;
use crate::compile_driver::_helpers::stages::deferral::deferred_locations;
use pyo3::prelude::{Bound, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_python_base::boundary::models::KeywordArguments;

/// Build physical locations for all models and seeds under a deferred target.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn build_deferred_locations<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyDict>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "build_deferred_locations",
        options,
        (
            &[
                "project",
                "deferred_target_config",
                "effective_vars",
                "default_schema",
                "default_database",
                "render_qualified_name",
            ],
            &[],
        ),
    )?;
    deferred_locations(
        &arguments.value("project")?,
        &DeferredNaming {
            target_config: arguments.value("deferred_target_config")?,
            effective_vars: arguments.value("effective_vars")?,
            default_schema: arguments.value("default_schema")?,
            default_database: arguments.value("default_database")?,
            render_qualified_name: arguments.value("render_qualified_name")?,
        },
    )
}
