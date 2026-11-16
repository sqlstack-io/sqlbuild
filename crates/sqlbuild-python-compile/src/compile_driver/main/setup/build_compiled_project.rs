//! Build one compiled project with adapter defaults and target validation applied.

use crate::compile_driver::_helpers::setup::project::ProjectRequest;
use crate::compile_driver::_helpers::setup::project::compiled_project;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_python_base::boundary::models::KeywordArguments;

/// Build one compiled project with adapter defaults and target validation applied.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn build_compiled_project<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "build_compiled_project",
        options,
        (
            &["discovered_inputs", "adapter"],
            &[
                "selected_target",
                "no_sql_validation",
                "skip_column_inference",
                "column_lineage_mode",
                "cli_vars",
                "external_sql_reference_resolver",
                "resolved_connection",
                "analysis_selection",
                "source_lexical_syntax",
            ],
        ),
    )?;
    compiled_project(py, ProjectRequest::read(&arguments)?)
}
