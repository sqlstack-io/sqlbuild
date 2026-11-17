//! Format build command execution results as JSON.

use crate::result_output::_helpers::results::documents::BUILD_OPTIONS;
use crate::result_output::_helpers::results::documents::build_json;
use pyo3::prelude::{Bound, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_host::models::BuildExecutionResult;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::types::ReadResult;

/// Format build command execution results as JSON.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn format_build_execution_json<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<String> {
    let arguments: KeywordArguments<'py> =
        KeywordArguments::read(py, "format_build_execution_json", options, BUILD_OPTIONS)?;
    let result = BuildExecutionResult::read(&arguments.value("result")?)?;
    build_json(py, &arguments, &result, Vec::new())
}
